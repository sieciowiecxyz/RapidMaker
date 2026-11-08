import { expect, test } from './fixtures';
import { request as httpRequest } from 'node:http';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';

// Guest cookies stay only in memory; request traces must never capture them.
test.use({ screenshot: 'off', trace: 'off', video: 'off' });

test('two unrelated clients complete uploads and private quotes after upload backpressure', async ({
  playwright,
}, testInfo) => {
  test.setTimeout(120_000);
  const origin = 'http://localhost:18437';
  const headers = {
    Origin: origin,
    'Content-Type': 'application/octet-stream',
    'x-model-filename': 'cube_20mm.stl',
  };
  const cube = readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
  const storage = process.env.RAPIDMAKER_TEST_MODEL_ROOT;
  if (!storage) throw new Error('Run with the isolated integration runner');
  const directories = () => (existsSync(storage) ? readdirSync(storage).sort() : []);
  const before = directories();
  const second = await playwright.request.newContext({ baseURL: origin });
  let first: typeof second | undefined;
  const models: { id: string; preview_url: string }[] = [];
  // Expect/continue synchronizes with body extraction after admission. Send a
  // valid prefix, hold the rest, then complete the legitimate body without timeout.
  const held = httpRequest('http://127.0.0.1:18436/api/models', {
    method: 'POST',
    headers: {
      ...headers,
      Host: 'localhost:18437',
      'Content-Length': cube.length,
      Expect: '100-continue',
    },
  });
  const completed = new Promise<{ status: number; cookies: string[]; body: Buffer }>(
    (resolveResult, reject) => {
      held.once('response', (response) => {
        const chunks: Buffer[] = [];
        response.on('data', (chunk: Buffer) => chunks.push(chunk));
        response.once('end', () =>
          resolveResult({
            status: response.statusCode ?? 0,
            cookies: response.headers['set-cookie'] ?? [],
            body: Buffer.concat(chunks),
          }),
        );
        response.once('error', () => reject(new Error('First upload response failed')));
      });
      held.once('error', () => reject(new Error('First upload connection failed')));
    },
  );
  void completed.catch(() => {});
  try {
    await new Promise<void>((ready, reject) => {
      held.once('continue', () => held.write(cube.subarray(0, 4), () => ready()));
      held.once('response', () => reject(new Error('First upload rejected before body')));
      held.once('error', () => reject(new Error('First upload could not start')));
      held.setTimeout(30_000, () => held.destroy(new Error('First upload timed out')));
      held.flushHeaders();
    });
    let secondBodyRequested = false;
    const rejected = httpRequest('http://127.0.0.1:18436/api/models', {
      method: 'POST',
      headers: {
        ...headers,
        Host: 'localhost:18437',
        'Content-Length': cube.length,
        Expect: '100-continue',
      },
    });
    let busyStatus: number;
    try {
      busyStatus = await new Promise<number>((resolveStatus, reject) => {
        rejected.once('continue', () => {
          secondBodyRequested = true;
          reject(new Error('Busy upload requested a body instead of rejecting admission'));
        });
        rejected.once('response', (response) => {
          response.resume();
          response.once('end', () => resolveStatus(response.statusCode ?? 0));
          response.once('error', () => reject(new Error('Busy response failed')));
        });
        rejected.once('error', () => reject(new Error('Busy upload connection failed')));
        rejected.setTimeout(10_000, () => rejected.destroy(new Error('Busy upload timed out')));
        rejected.flushHeaders();
      });
    } finally {
      rejected.destroy();
    }
    expect(busyStatus).toBe(429);
    expect(secondBodyRequested).toBe(false);
    // Neither the incomplete first body nor the refused second creates model files.
    expect(directories()).toEqual(before);
    const catalogStarted = performance.now();
    const catalog = await second.get('/api/catalog/materials', { timeout: 5000 });
    const catalogMs = performance.now() - catalogStarted;
    expect(catalog.status()).toBe(200);
    expect(catalogMs).toBeLessThan(5000);
    const material = (await catalog.json()).materials.find(
      (value: { id: string }) => value.id === 'GFA00',
    );
    expect(material?.colors.length).toBeGreaterThan(0);

    held.end(cube.subarray(4));
    const uploaded = await completed;
    expect(uploaded.status).toBe(201);
    // Import only the raw response's cookie pairs into the first private jar.
    // No cookie values appear in assertions, attachments or storageState files.
    const cookies = uploaded.cookies.map((header) => {
      const pair = header.split(';', 1)[0];
      const separator = pair.indexOf('=');
      if (separator < 1) throw new Error('Upload returned an invalid cookie');
      return {
        name: pair.slice(0, separator),
        value: pair.slice(separator + 1),
        domain: 'localhost',
        path: '/',
        expires: -1,
        httpOnly: true,
        secure: false,
        sameSite: 'Lax' as const,
      };
    });
    if (!cookies.length) throw new Error('First upload did not establish a guest session');
    first = await playwright.request.newContext({
      baseURL: origin,
      storageState: { cookies, origins: [] },
    });
    models.push(JSON.parse(uploaded.body.toString()));
    const retry = await second.post('/api/models', { headers, data: cube });
    expect(retry.status()).toBe(201);
    models.push(await retry.json());
    expect(models[0].id).not.toBe(models[1].id);
    const clients = [first, second];
    for (let index = 0; index < clients.length; index++) {
      const sourcePath = `/api/models/${models[index].id}/source`;
      const source = await clients[index].get(sourcePath);
      expect(source.status()).toBe(200);
      expect(await source.body()).toEqual(cube);
      expect((await clients[1 - index].get(sourcePath)).status()).toBe(404);
      const preview = await clients[index].get(models[index].preview_url);
      expect(preview.status()).toBe(200);
      expect(preview.headers()['cache-control']).toContain('no-store');
      expect((await preview.body()).subarray(0, 4).toString()).toBe('glTF');
      expect((await clients[1 - index].get(models[index].preview_url)).status()).toBe(404);
    }
    const jobs = await Promise.all(
      clients.map(async (client, index) => {
        const response = await client.post('/api/quotes/estimate', {
          headers: { Origin: origin },
          data: {
            model_id: models[index].id,
            material_id: material.id,
            color_code: material.colors[0].code,
            options: {
              quantity: index + 1,
              quality: 'standard',
              wall_loops: 3,
              infill_percent: 20,
            },
          },
        });
        expect(response.status()).toBe(202);
        return response.json();
      }),
    );
    expect(jobs[0].id).not.toBe(jobs[1].id);
    const prices = await Promise.all(
      clients.map(async (client, index) => {
        expect((await clients[1 - index].get(jobs[index].poll_url)).status()).toBe(404);
        let job = jobs[index];
        await expect
          .poll(
            async () => {
              const response = await client.get(job.poll_url);
              expect(response.status()).toBe(200);
              job = await response.json();
              return job.status;
            },
            { timeout: 80_000, intervals: [500, 1000] },
          )
          .toMatch(/^(succeeded|failed)$/);
        expect(job.status, 'Real owned slicer job must succeed').toBe('succeeded');
        expect(job.result).toMatchObject({
          id: jobs[index].id,
          model_id: models[index].id,
          quantity: index + 1,
        });
        expect(job.result.price.gross_grosze).toBeGreaterThan(0);
        return job.result.price.gross_grosze;
      }),
    );
    await testInfo.attach('two-client-upload-evidence', {
      contentType: 'application/json',
      body: JSON.stringify({
        busy_status: busyStatus,
        refused_before_body: !secondBodyRequested,
        catalog_status: catalog.status(),
        catalog_ms: Math.round(catalogMs),
        upload_statuses: [uploaded.status, retry.status()],
        owned_prices_grosze: prices,
        isolated_previews_and_quotes: true,
      }),
    });
  } finally {
    held.destroy();
    const clients = [first, second];
    for (let index = 0; index < models.length; index++) {
      await clients[index]?.delete(`/api/models/${models[index].id}`, { headers }).catch(() => {});
    }
    await first?.dispose();
    await second.dispose();
    // The runner also disposes this run's whole storage/DB, including failed jobs.
  }
});
