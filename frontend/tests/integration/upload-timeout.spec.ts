import { expect, test } from './fixtures';
import { request as httpRequest } from 'node:http';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

test.use({ screenshot: 'off', trace: 'off', video: 'off' });

test('invalid filename cannot hold upload admission while its body stalls', async ({ request }) => {
  const source = readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
  const origin = { Origin: 'http://localhost:18437' };
  const waiting = httpRequest('http://127.0.0.1:18436/api/models', {
    method: 'POST',
    headers: {
      ...origin,
      Host: 'localhost:18437',
      'x-model-filename': 'unsupported.exe',
      'Content-Length': source.length,
    },
  });
  const refused = new Promise<number>((done) => {
    waiting.once('response', (response) => {
      response.resume();
      response.once('end', () => done(response.statusCode ?? 0));
    });
    waiting.once('error', () => done(0));
  });
  let created: string | undefined;
  try {
    waiting.write(source.subarray(0, 4)); // Deliberately leave the declared body unfinished.
    const earlyStatus = await Promise.race([
      refused,
      new Promise<null>((done) => setTimeout(() => done(null), 500)),
    ]);
    const healthy = await request.post('/api/models', {
      headers: { ...origin, 'x-model-filename': 'cube_20mm.stl' },
      data: source,
    });
    if (healthy.status() === 201) created = (await healthy.json()).id;
    await test.info().attach('early-upload-rejection', {
      body: JSON.stringify({ earlyStatus, healthyStatus: healthy.status() }),
      contentType: 'application/json',
    });
    expect(earlyStatus).toBe(400);
    expect(healthy.status()).toBe(201);
  } finally {
    waiting.destroy();
    if (created) await request.delete(`/api/models/${created}`, { headers: origin });
  }
});

test('stalled upload expires, releases admission and permits a private model upload', async ({
  request,
  playwright,
}, testInfo) => {
  test.setTimeout(120_000);
  const headers = {
    Origin: 'http://localhost:18437',
    'Content-Type': 'application/octet-stream',
    'x-model-filename': 'cube_20mm.stl',
  };
  const cube = readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
  const created: string[] = [];
  const started = performance.now();
  // Exercise the backend deadline directly; recovery below goes through Vite.
  const waiting = httpRequest('http://127.0.0.1:18436/api/models', {
    method: 'POST',
    headers: {
      ...headers,
      Host: 'localhost:18437',
      'Content-Length': 1024,
      Expect: '100-continue',
    },
  });
  // Install both listeners before flushing; a failed socket cannot create an
  // unhandled rejection while the admission check is running.
  const result = new Promise<{ status: number; elapsedSeconds: number }>(
    (resolveResult, reject) => {
      waiting.once('response', (response) => {
        response.resume();
        response.once('end', () =>
          resolveResult({
            status: response.statusCode ?? 0,
            elapsedSeconds: (performance.now() - started) / 1000,
          }),
        );
        response.once('error', () => reject(new Error('Stalled upload response failed')));
      });
      waiting.once('error', () => reject(new Error('Stalled upload connection failed')));
    },
  );
  void result.catch(() => {});
  try {
    await new Promise<void>((resolveReady, reject) => {
      waiting.once('continue', () => {
        waiting.write('soli', () => resolveReady());
      });
      waiting.once('error', (error: NodeJS.ErrnoException) =>
        reject(new Error(`Stalled upload could not start: ${error.code ?? error.name}`)),
      );
      waiting.once('response', () =>
        reject(new Error('Upload responded before its body was sent')),
      );
      waiting.setTimeout(95_000, () =>
        waiting.destroy(new Error('Stalled upload exceeded deadline')),
      );
      waiting.flushHeaders();
    });
    await expect
      .poll(
        async () => {
          const concurrent = await request.post('/api/models', { headers, data: cube });
          // A possible initial race creates a valid, owned model; always remove it.
          if (concurrent.status() === 201) {
            const model = await concurrent.json();
            created.push(model.id);
          }
          return concurrent.status();
        },
        { timeout: 10_000 },
      )
      .toBe(429);
    expect((await request.get('/api/models/limits')).status()).toBe(200);
    const timeout = await result;
    await testInfo.attach('upload-deadline-status', {
      body: JSON.stringify({
        stalledStatus: timeout.status,
        elapsedSeconds: Number(timeout.elapsedSeconds.toFixed(3)),
        concurrentStatus: 429,
        limitsStatus: 200,
      }),
      contentType: 'application/json',
    });
    expect(timeout.status).toBe(408);
    expect(timeout.elapsedSeconds).toBeGreaterThanOrEqual(55);
    expect(timeout.elapsedSeconds).toBeLessThan(90);
    const recovered = await request.post('/api/models', { headers, data: cube });
    expect(recovered.status()).toBe(201);
    const model = await recovered.json();
    created.push(model.id);
    const preview = await request.get(model.preview_url);
    expect(preview.status()).toBe(200);
    expect(preview.headers()['cache-control']).toContain('no-store');
    expect((await preview.body()).subarray(0, 4).toString()).toBe('glTF');
    const outsider = await playwright.request.newContext({ baseURL: 'http://localhost:18437' });
    try {
      expect((await outsider.get(model.preview_url)).status()).toBe(401);
    } finally {
      await outsider.dispose();
    }
    expect((await request.delete(`/api/models/${model.id}`, { headers })).status()).toBe(204);
    created.splice(created.indexOf(model.id), 1);
  } finally {
    waiting.destroy();
    for (const id of created) {
      await request.delete(`/api/models/${id}`, { headers }).catch(() => {});
    }
  }
});
