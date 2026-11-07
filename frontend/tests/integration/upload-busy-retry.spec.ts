import { expect, test, type Page } from './fixtures';
import { request as httpRequest } from 'node:http';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';

test.use({ screenshot: 'off', trace: 'off', video: 'off', reducedMotion: 'reduce' });
const cube = resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl');
const source = readFileSync(cube);
const isUpload = (r: { url(): string; request(): { method(): string } }) =>
  new URL(r.url()).pathname === '/api/models' && r.request().method() === 'POST';
const input = (page: Page) => page.getByLabel('Dodaj modele STL, STEP lub 3MF');
async function open(page: Page, advanced = false) {
  await page.goto(advanced ? '/wycena/zaawansowana' : '/wycena');
  const field = advanced ? page.getByLabel('Wybierz modele', { exact: true }) : input(page);
  await expect(field).toBeEnabled();
  return field;
}

// A separate genuine upload holds admission before consuming its complete body.
async function holdUpload() {
  const held = httpRequest('http://127.0.0.1:18436/api/models', {
    method: 'POST',
    headers: {
      Origin: 'http://localhost:18437',
      Host: 'localhost:18437',
      'Content-Type': 'application/octet-stream',
      'x-model-filename': 'cube_20mm.stl',
      'Content-Length': source.length,
      Expect: '100-continue',
    },
  });
  const completed = new Promise<number>((done, reject) => {
    held.once('response', (response) => {
      response.resume();
      response.once('end', () => done(response.statusCode ?? 0));
      response.once('error', reject);
    });
    held.once('error', reject);
  });
  void completed.catch(() => {});
  await new Promise<void>((ready, reject) => {
    held.once('continue', () => held.write(source.subarray(0, 4), () => ready()));
    held.once('response', () => reject(new Error('Held upload refused')));
    held.once('error', reject);
    held.setTimeout(45_000, () => held.destroy(new Error('Held upload timeout')));
    held.flushHeaders();
  });
  return { held, completed };
}

for (const advanced of [false, true]) {
  test(`real busy refusal retries to ready in ${advanced ? 'advanced' : 'storefront'}`, async ({
    page,
  }) => {
    const field = await open(page, advanced);
    const root = process.env.RAPIDMAKER_TEST_MODEL_ROOT!;
    const before = readdirSync(root).sort();
    const { held, completed } = await holdUpload();
    let attempts = 0;
    page.on('request', (r) => {
      if (isUpload({ url: () => r.url(), request: () => r })) attempts++;
    });
    try {
      const probe = await page.request.post('/api/models', {
        headers: { Origin: 'http://localhost:18437', 'x-model-filename': 'cube_20mm.stl' },
        data: source,
      });
      expect(probe.status()).toBe(429);
      expect((await probe.json()).code).toBe('model_upload_busy');
      expect(probe.headers()['retry-after']).toBe('1');
      const busy = page.waitForResponse(isUpload);
      await field.setInputFiles(cube);
      const refusal = await busy;
      expect(refusal.status()).toBe(429);
      expect(refusal.headers()['retry-after']).toBe('1');
      expect(readdirSync(root).sort()).toEqual(before);
      const accepted = page.waitForResponse((r) => isUpload(r) && r.status() === 201);
      held.end(source.subarray(4));
      expect(await completed).toBe(201);
      await accepted;
      // Chromium may evict inspector response bodies. Read the actual application's
      // accepted ID after its normal adapter persists the successful result.
      const acceptedId = () =>
        page.evaluate((isAdvanced) => {
          const state = JSON.parse(
            localStorage.getItem(
              isAdvanced ? 'rapidmaker.frontend.demo.v1' : 'rapidmaker.storefront.draft.v1',
            ) ?? 'null',
          );
          return (isAdvanced ? state?.cart?.models : state?.models)?.find(
            (model: { name: string }) => model.name === 'cube_20mm.stl',
          )?.analysis?.id;
        }, advanced);
      await expect.poll(acceptedId).toBeTruthy();
      const id = await acceptedId();
      expect((await page.request.get(`/api/models/${id}`)).status()).toBe(200);
      expect(
        Buffer.compare(await (await page.request.get(`/api/models/${id}/source`)).body(), source),
      ).toBe(0);
      if (advanced)
        await expect(
          page.getByRole('heading', { name: 'cube_20mm.stl', exact: true }),
        ).toBeVisible();
      else
        await expect
          .poll(() =>
            page.evaluate(
              () =>
                JSON.parse(localStorage.getItem('rapidmaker.storefront.draft.v1') ?? 'null')
                  ?.models?.[0]?.status,
            ),
          )
          .toBe('ready');
      expect(attempts).toBe(2);
      expect(readdirSync(root).length).toBe(before.length + 2);
    } finally {
      held.destroy();
    }
  });
}

test('persistent marked busy stops with eight attempts within thirty seconds', async ({ page }) => {
  test.setTimeout(40_000);
  const field = await open(page);
  let attempts = 0;
  await page.route('**/api/models', (route) => {
    attempts++;
    return route.fulfill({
      status: 429,
      headers: { 'Retry-After': '1' },
      json: { code: 'model_upload_busy', message: 'Controlled persistent busy' },
    });
  });
  const started = performance.now();
  await field.setInputFiles(cube);
  await expect(page.getByRole('alert')).toContainText('Controlled persistent busy', {
    timeout: 30_000,
  });
  expect(performance.now() - started).toBeLessThan(30_000);
  expect(attempts).toBe(8);
  await page.waitForTimeout(1300);
  expect(attempts).toBe(8);
});

for (const [status, code, retryAfter] of [
  [429, undefined, '1'],
  [429, 'model_upload_busy', '2'],
  [503, 'model_upload_busy', '1'],
  [403, undefined, '1'],
  [401, undefined, '1'],
  [429, 'other_busy', '1'],
] as const) {
  test(`controlled non-retry response ${status}/${code}/${retryAfter} has one attempt`, async ({
    page,
  }) => {
    const field = await open(page);
    let attempts = 0;
    await page.route('**/api/models', (route) => {
      attempts++;
      return route.fulfill({
        status,
        headers: { 'Retry-After': retryAfter },
        json: { code, message: 'Controlled refusal' },
      });
    });
    await field.setInputFiles(cube);
    await expect(page.getByRole('alert')).toContainText('Controlled refusal');
    await page.waitForTimeout(1400);
    expect(attempts).toBe(1);
  });
}

test('removing a busy local row stops queued resends without displaying its error', async ({
  page,
}) => {
  const field = await open(page);
  let attempts = 0;
  await page.route('**/api/models', (route) => {
    attempts++;
    return route.fulfill({
      status: 429,
      headers: { 'Retry-After': '1' },
      json: { code: 'model_upload_busy', message: 'Controlled busy removal' },
    });
  });
  const refusal = page.waitForResponse(isUpload);
  await field.setInputFiles(cube);
  await refusal;
  await page.getByRole('button', { name: 'Usuń cube_20mm.stl', exact: true }).click();
  await page.waitForTimeout(1600);
  expect(attempts).toBe(1);
  await expect(page.getByRole('alert')).toHaveCount(0);
});

// A transport failure may have accepted a body; it must never resend automatically.
test('controlled uncertain network failure is not retried', async ({ page }) => {
  const field = await open(page);
  let attempts = 0;
  await page.route('**/api/models', (route) => {
    attempts++;
    return route.abort('connectionreset');
  });
  await field.setInputFiles(cube);
  await expect(page.getByRole('alert')).toBeVisible();
  await page.waitForTimeout(1400);
  expect(attempts).toBe(1);
});

// Real filesystem contention, separate from the pre-body process semaphore.
test('root flock refusal is explicitly retryable and creates no upload files', async ({
  request,
}) => {
  const root = process.env.RAPIDMAKER_TEST_MODEL_ROOT!;
  const before = readdirSync(root).sort();
  const lock = spawn(
    'python',
    [
      '-u',
      '-c',
      "import fcntl,os,sys; fd=os.open(sys.argv[1],os.O_RDONLY); fcntl.flock(fd,fcntl.LOCK_EX); print('locked',flush=True); sys.stdin.read()",
      root,
    ],
    { stdio: ['pipe', 'pipe', 'pipe'] },
  );
  const exit = once(lock, 'exit');
  try {
    await once(lock.stdout, 'data');
    const response = await request.post('/api/models', {
      headers: { Origin: 'http://localhost:18437', 'x-model-filename': 'cube_20mm.stl' },
      data: source,
    });
    expect(response.status()).toBe(429);
    expect((await response.json()).code).toBe('model_upload_busy');
    expect(response.headers()['retry-after']).toBe('1');
    expect(readdirSync(root).sort()).toEqual(before);
  } finally {
    lock.stdin.end();
    await exit;
  }
});
