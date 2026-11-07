import { expect, test, type Page } from './fixtures';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { accounts, login } from './helpers';
import { existsSync, readFileSync, realpathSync, statSync } from 'node:fs';
import { basename, dirname, extname, resolve, sep } from 'node:path';

test.use({ screenshot: 'off', trace: 'off', reducedMotion: 'reduce' });
const origin = 'http://localhost:18437';
const cube = resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl');
const input = (page: Page) => page.getByLabel('Dodaj modele STL, STEP lub 3MF');
const row = (page: Page, name = 'cube_20mm.stl') =>
  page.getByRole('button', { name: `Konfiguruj ${name}`, exact: true });
const remove = (page: Page) =>
  page.getByRole('button', { name: 'Usuń cube_20mm.stl', exact: true });
const isUpload = (response: { url(): string; request(): { method(): string } }) =>
  new URL(response.url()).pathname === '/api/models' && response.request().method() === 'POST';
const deletion = (page: Page, id: string) =>
  page.waitForResponse(
    (r) => new URL(r.url()).pathname === `/api/models/${id}` && r.request().method() === 'DELETE',
  );
function directory(id: string) {
  expect(id).toMatch(/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/);
  const root = process.env.RAPIDMAKER_TEST_MODEL_ROOT ?? '';
  expect(root).toMatch(/^\/tmp\/rapidmaker-browser-[^/]+\/models$/);
  expect(realpathSync(root)).toBe(root);
  return resolve(root, id);
}
async function open(page: Page) {
  await page.goto('/wycena');
  const cap = process.env.RAPIDMAKER_LARGE_MODELS === '1' ? 96 : 32;
  await expect(
    page.getByText(`STL, STEP, 3MF / do ${cap} MB na plik`, { exact: true }),
  ).toBeVisible();
}
async function upload(page: Page, path = cube) {
  const response = page.waitForResponse(isUpload, { timeout: 90_000 });
  await input(page).setInputFiles(path);
  expect((await response).status()).toBe(201);
  const id = () =>
    page.evaluate(
      (name) =>
        JSON.parse(localStorage.getItem('rapidmaker.storefront.draft.v1') ?? 'null')?.models?.find(
          (m: { name: string }) => m.name === name,
        )?.analysis?.id,
      basename(path),
    );
  await expect.poll(id).toBeTruthy();
  return (await id()) as string;
}
async function gone(page: Page, id: string) {
  await expect.poll(() => existsSync(directory(id))).toBe(false);
  expect((await page.request.get(`/api/models/${id}`)).status()).toBe(404);
}
async function heldUpload(page: Page) {
  let release!: () => void, accepted!: (id: string) => void;
  const gate = new Promise<void>((resolveGate) => {
    release = resolveGate;
  });
  const committed = new Promise<string>((resolveId) => {
    accepted = resolveId;
  });
  // Controlled response delay only: upload reaches the real server and commits before UI removal.
  await page.route(
    '**/api/models',
    async (route) => {
      const response = await route.fetch();
      expect(response.status()).toBe(201);
      accepted((await response.json()).id);
      await gate;
      await route.fulfill({ response });
    },
    { times: 1 },
  );
  return { committed, release };
}

test('ready UI removal deletes real files and rejects non-owner mutations', async ({
  page,
  playwright,
}) => {
  await open(page);
  const id = await upload(page);
  expect(existsSync(directory(id))).toBe(true);
  expect((await page.request.delete(`/api/models/${id}`)).status()).toBe(403);
  expect(
    (
      await page.request.delete(`/api/models/${id}`, {
        headers: { Origin: 'https://evil.example.invalid' },
      })
    ).status(),
  ).toBe(403);
  const outsider = await playwright.request.newContext({ baseURL: origin });
  try {
    expect(
      (await outsider.delete(`/api/models/${id}`, { headers: { Origin: origin } })).status(),
    ).toBe(401);
    const other = await outsider.post('/api/models', {
      headers: {
        Origin: origin,
        'Content-Type': 'application/octet-stream',
        'x-model-filename': 'cube_20mm.stl',
      },
      data: readFileSync(cube),
    });
    expect(other.status()).toBe(201);
    expect(
      (await outsider.delete(`/api/models/${id}`, { headers: { Origin: origin } })).status(),
    ).toBe(404);
    expect((await page.request.get(`/api/models/${id}`)).status()).toBe(200);
    expect(existsSync(directory(id))).toBe(true);
    const deleted = deletion(page, id);
    await remove(page).click();
    expect((await deleted).status()).toBe(204);
    await expect(row(page)).toHaveCount(0);
    await gone(page, id);
  } finally {
    await outsider.dispose();
  }
});

test('duplicate variants keep independent settings and release only the last upload reference', async ({
  page,
}) => {
  test.setTimeout(240_000);
  await open(page);
  let uploads = 0,
    deletes = 0;
  page.on('request', (request) => {
    if (new URL(request.url()).pathname === '/api/models' && request.method() === 'POST') uploads++;
    if (
      /\/api\/models\/[^/]+$/.test(new URL(request.url()).pathname) &&
      request.method() === 'DELETE'
    )
      deletes++;
  });
  const id = await upload(page);
  const database = process.env.RAPIDMAKER_TEST_DATABASE ?? '';
  expect(database).toMatch(/^rapidmaker_test_browser_[a-f0-9]+$/);
  const quota = () =>
    execFileSync(
      'podman',
      [
        'exec',
        process.env.TEST_PG_CONTAINER ?? 'rapidmaker-catalog-postgres',
        'psql',
        '-U',
        'rapidmaker',
        '-d',
        database,
        '-v',
        'ON_ERROR_STOP=1',
        '-At',
        '-c',
        `SELECT count(*), COALESCE(sum(source_bytes+mesh_bytes+preview_bytes),0) FROM private_models WHERE id='${id}'`,
      ],
      { encoding: 'utf8', timeout: 15_000, stdio: 'pipe' },
    ).trim();
  expect(quota()).toMatch(/^1\|[1-9][0-9]*$/);
  for (const [name, value] of [
    ['material', 'pla'],
    ['color', 'black'],
    ['quality', 'standard'],
    ['strength', 'standard'],
  ])
    await page.locator(`input[name=${name}][value=${value}]`).check();
  const proceed = page.getByRole('button', { name: 'Przejdź do zamówienia', exact: true });
  await expect(proceed).toBeEnabled({ timeout: 90_000 });
  const rows = page.locator('.model-row');
  const duplicate = page.getByRole('button', { name: 'Powiel wariant cube_20mm.stl', exact: true });
  await rows.first().getByRole('spinbutton').fill('2');
  for (let count = 1; count < 10; count++) await duplicate.first().click();
  await expect(rows).toHaveCount(10);
  for (let index = 0; index < 10; index++) await expect(duplicate.nth(index)).toBeDisabled();
  for (let count = 10; count > 2; count--) await remove(page).last().click();
  await expect(rows).toHaveCount(2);
  expect(deletes).toBe(0);
  await expect(rows.nth(1).getByRole('spinbutton')).toHaveValue('2');
  await rows.nth(1).getByRole('spinbutton').fill('3');
  await rows.nth(1).getByRole('button', { name: 'Konfiguruj cube_20mm.stl' }).click();
  await page.locator('input[name=color][value=white]').check();
  await page.locator('input[name=quality][value=fine]').check();
  await page.locator('input[name=strength][value=reinforced]').check();
  await expect(proceed).toBeEnabled({ timeout: 90_000 });
  await page.reload();
  await expect(rows.nth(0)).toContainText('Czarny');
  await expect(rows.nth(1)).toContainText('Biały');
  await expect(rows.nth(0).getByRole('spinbutton')).toHaveValue('2');
  await expect(rows.nth(1).getByRole('spinbutton')).toHaveValue('3');
  await page
    .locator('.models')
    .screenshot({ path: test.info().outputPath('variants-desktop.png') });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.locator('.models').screenshot({ path: test.info().outputPath('variants-mobile.png') });
  await page.setViewportSize({ width: 1440, height: 1000 });
  const draft = await page.evaluate(() =>
    JSON.parse(localStorage.getItem('rapidmaker.storefront.draft.v1')!),
  );
  expect(new Set(draft.models.map((model: { id: string }) => model.id)).size).toBe(2);
  expect(draft.models.map((model: { analysis: { id: string } }) => model.analysis.id)).toEqual([
    id,
    id,
  ]);
  expect(
    draft.models.map((model: { settings: { quality: string } }) => model.settings.quality),
  ).toEqual(['standard', 'fine']);
  await expect(proceed).toBeEnabled({ timeout: 90_000 });
  await proceed.click();
  await expect(page.locator('.order-summary li')).toHaveCount(2);
  await page.goto('/wycena?restore=cart');
  await expect(rows.nth(0)).toContainText('Czarny');
  await expect(rows.nth(1)).toContainText('Biały');
  await expect(rows.nth(0).getByRole('spinbutton')).toHaveValue('2');
  await expect(rows.nth(1).getByRole('spinbutton')).toHaveValue('3');
  await page.goto('/wycena/zaawansowana');
  await remove(page).click();
  await expect(page.locator('.part-row')).toHaveCount(1);
  expect((await page.request.get(`/api/models/${id}`)).status()).toBe(200);
  expect(existsSync(directory(id))).toBe(true);
  expect(deletes).toBe(0);
  await page.getByRole('link', { name: '← Wróć do konfiguratora', exact: true }).click();
  await expect(rows).toHaveCount(1);
  await expect(rows).toContainText('Biały');
  await expect(rows.getByRole('spinbutton')).toHaveValue('3');
  await expect(proceed).toBeEnabled({ timeout: 90_000 });
  const deleted = deletion(page, id);
  await remove(page).click();
  expect((await deleted).status()).toBe(204);
  await expect(rows).toHaveCount(0);
  await gone(page, id);
  expect(deletes).toBe(1);
  expect(quota()).toBe('0|0');
  // A real upload still fits after physical storage and its quota record are released.
  const replacement = await upload(page);
  expect(replacement).not.toBe(id);
  expect(uploads).toBe(2);
  const cleanup = deletion(page, replacement);
  await remove(page).click();
  expect((await cleanup).status()).toBe(204);
  await gone(page, replacement);
});

test('controlled DELETE 503 keeps the ready model and real retry removes it', async ({ page }) => {
  await open(page);
  const id = await upload(page);
  const url = `**/api/models/${id}`;
  const message = 'Kontrolowana awaria usuwania modelu.';
  await page.route(url, (route) => route.fulfill({ status: 503, json: { message } }));
  await remove(page).click();
  await expect(page.getByRole('alert')).toHaveText(message);
  await expect(row(page)).toBeVisible();
  expect(existsSync(directory(id))).toBe(true);
  expect((await page.request.get(`/api/models/${id}`)).status()).toBe(200);
  await page.unroute(url);
  const deleted = deletion(page, id);
  await remove(page).click();
  expect((await deleted).status()).toBe(204);
  await gone(page, id);
});

for (const action of ['remove', 'sample'] as const) {
  test(`late accepted upload is released after UI ${action}`, async ({ page }) => {
    await open(page);
    const held = await heldUpload(page);
    try {
      await input(page).setInputFiles(cube);
      const id = await held.committed;
      expect(existsSync(directory(id))).toBe(true);
      if (action === 'remove') await remove(page).click();
      else {
        page.once('dialog', (dialog) => dialog.accept());
        await page.getByRole('button', { name: 'Wczytaj ponownie przykład', exact: true }).click();
      }
      await expect(row(page)).toHaveCount(0);
      const deleted = deletion(page, id);
      held.release();
      expect((await deleted).status()).toBe(204);
      await gone(page, id);
      await expect(row(page)).toHaveCount(0);
      if (action === 'sample') await expect(row(page, 'uchwyt-montazowy.stl')).toBeVisible();
    } finally {
      held.release();
    }
  });
}

test('failed late cleanup is visible and retried before the next real upload', async ({ page }) => {
  await open(page);
  const held = await heldUpload(page);
  try {
    await input(page).setInputFiles(cube);
    const id = await held.committed;
    const message = 'Kontrolowana awaria sprzątania spóźnionego uploadu.';
    await page.route(
      `**/api/models/${id}`,
      (route) => route.fulfill({ status: 503, json: { message } }),
      { times: 1 },
    );
    await remove(page).click();
    await expect(row(page)).toHaveCount(0);
    held.release();
    await expect(page.getByRole('alert')).toHaveText(message);
    expect(existsSync(directory(id))).toBe(true);
    const order: string[] = [];
    page.on('request', (request) => {
      if (new URL(request.url()).pathname === `/api/models/${id}` && request.method() === 'DELETE')
        order.push('DELETE');
      if (new URL(request.url()).pathname === '/api/models' && request.method() === 'POST')
        order.push('POST');
    });
    const deleted = deletion(page, id);
    const next = await upload(page);
    expect((await deleted).status()).toBe(204);
    expect(order).toEqual(['DELETE', 'POST']);
    expect(next).not.toBe(id);
    await gone(page, id);
    expect(existsSync(directory(next))).toBe(true);
  } finally {
    held.release();
  }
});

test('opt-in large removal returns guest quota before the next corpus upload', async ({ page }) => {
  test.skip(
    process.env.RAPIDMAKER_LARGE_MODELS !== '1',
    'Requires --large-models and explicit private corpus manifest',
  );
  test.setTimeout(300_000);
  const manifest = process.env.E2E_CORPUS_MANIFEST;
  if (!manifest) throw new Error('--large-models requires E2E_CORPUS_MANIFEST');
  const cases: { case: number; file: string; size_bytes: number; sha256: string }[] = JSON.parse(
    readFileSync(manifest, 'utf8'),
  );
  const root = realpathSync(dirname(resolve(manifest)));
  const source = (number: number) => {
    const matches = cases.filter((item) => item.case === number);
    expect(matches).toHaveLength(1);
    const item = matches[0],
      path = realpathSync(resolve(root, item.file));
    expect(path.startsWith(root + sep)).toBe(true);
    expect(item.sha256).toMatch(/^[a-f0-9]{64}$/);
    expect(item.size_bytes).toBeLessThanOrEqual(96 * 1024 * 1024);
    expect(statSync(path).isFile()).toBe(true);
    expect(statSync(path).size).toBe(item.size_bytes);
    expect(createHash('sha256').update(readFileSync(path)).digest('hex')).toBe(item.sha256);
    return path;
  };
  await open(page);
  const firstPath = source(8),
    secondPath = source(20);
  const first = await upload(page, firstPath);
  const totalBytes = (id: string, path: string) =>
    ['model' + extname(path).toLowerCase(), 'canonical.stl', 'preview.glb'].reduce(
      (sum, name) => sum + statSync(resolve(directory(id), name)).size,
      0,
    );
  const firstBytes = totalBytes(first, firstPath);
  const deleted = deletion(page, first);
  await page.getByRole('button', { name: `Usuń ${basename(firstPath)}`, exact: true }).click();
  expect((await deleted).status()).toBe(204);
  await gone(page, first);
  const second = await upload(page, secondPath);
  expect(firstBytes + totalBytes(second, secondPath)).toBeGreaterThan(256 * 1024 * 1024);
  expect((await page.request.get(`/api/models/${second}`)).status()).toBe(200);
});

test('account model removal requires both a valid origin and its session CSRF token', async ({
  page,
}) => {
  test.skip(!process.env.RAPIDMAKER_TEST_ACCOUNTS, 'Requires --local-accounts');
  await login(
    page,
    accounts().find((account) => account.email === 'browser-security@example.invalid')!,
  );
  await open(page);
  const id = await upload(page);
  const me = await (await page.request.get('/api/auth/me')).json();
  expect(
    (await page.request.delete(`/api/models/${id}`, { headers: { Origin: origin } })).status(),
  ).toBe(403);
  expect(
    (
      await page.request.delete(`/api/models/${id}`, {
        headers: { Origin: origin, 'X-CSRF-Token': 'invalid' },
      })
    ).status(),
  ).toBe(403);
  expect(
    (
      await page.request.delete(`/api/models/${id}`, {
        headers: { Origin: 'https://example.invalid', 'X-CSRF-Token': me.csrf_token },
      })
    ).status(),
  ).toBe(403);
  expect((await page.request.get(`/api/models/${id}`)).status()).toBe(200);
  expect(existsSync(directory(id))).toBe(true);
  expect(
    (
      await page.request.delete(`/api/models/${id}`, {
        headers: { Origin: origin, 'X-CSRF-Token': me.csrf_token },
      })
    ).status(),
  ).toBe(204);
  await gone(page, id);
});

test('authenticated removal recognizes its unclaimed guest cookie without claiming other files', async ({
  page,
  context,
  playwright,
}) => {
  test.skip(!process.env.RAPIDMAKER_TEST_ACCOUNTS, 'Requires --local-accounts');
  const guest = await playwright.request.newContext({ baseURL: origin });
  try {
    const send = () =>
      guest.post('/api/models', {
        headers: {
          Origin: origin,
          'x-model-filename': 'cube_20mm.stl',
          'Content-Type': 'application/octet-stream',
        },
        data: readFileSync(cube),
      });
    const firstResponse = await send();
    expect(firstResponse.status()).toBe(201);
    const first = await firstResponse.json();
    const secondResponse = await send();
    expect(secondResponse.status()).toBe(201);
    const second = await secondResponse.json();
    await login(
      page,
      accounts().find((account) => account.email === 'browser-security-negative@example.invalid')!,
    );
    // Reproduce a guest identity arriving after login/claim (e.g. a late upload
    // response); do not expose cookie contents in reports or API arguments.
    await context.addCookies(
      (await guest.storageState()).cookies.filter((cookie) => cookie.name === 'rapidmaker_guest'),
    );
    const me = await (await page.request.get('/api/auth/me')).json();
    expect((await page.request.get(`/api/models/${first.id}`)).status()).toBe(404);
    expect(
      (
        await page.request.delete(`/api/models/${first.id}`, {
          headers: { Origin: origin, 'X-CSRF-Token': me.csrf_token },
        })
      ).status(),
    ).toBe(204);
    await gone(page, first.id);
    expect((await guest.get(`/api/models/${second.id}`)).status()).toBe(200);
    expect((await page.request.get(`/api/models/${second.id}`)).status()).toBe(404);
  } finally {
    await guest.dispose();
  }
});

for (const cleanupFails of [false, true]) {
  test(`advanced configurator releases accepted uploads after local failure (retry=${cleanupFails})`, async ({
    page,
  }) => {
    await page.goto('/wycena/zaawansowana');
    await expect(page.getByLabel('Wybierz modele', { exact: true })).toBeEnabled();
    let id = '';
    // Capture the real accepted ID before forwarding the response: Chromium can
    // evict inspector bodies while the application immediately cleans up.
    await page.route(
      '**/api/models',
      async (route) => {
        const response = await route.fetch();
        expect(response.status()).toBe(201);
        id = (await response.json()).id;
        await route.fulfill({ response });
      },
      { times: 1 },
    );
    await page.route('**/api/catalog/materials', (route) =>
      route.fulfill({ status: 503, json: { message: 'Test: katalog chwilowo niedostępny.' } }),
    );
    if (cleanupFails)
      await page.route(
        /\/api\/models\/[^/]+$/,
        (route) =>
          route.request().method() === 'DELETE'
            ? route.fulfill({ status: 503, json: { message: 'Test: sprzątanie niedostępne.' } })
            : route.fallback(),
        { times: 1 },
      );
    const accepted = page.waitForResponse(isUpload);
    await page.getByLabel('Wybierz modele', { exact: true }).setInputFiles(cube);
    expect((await accepted).status()).toBe(201);
    await expect(page.getByRole('alert')).toContainText('Test: katalog chwilowo niedostępny.');
    if (cleanupFails) {
      await expect(page.getByRole('alert')).toContainText('Test: sprzątanie niedostępne.');
      expect(existsSync(directory(id))).toBe(true);
      expect((await page.request.get(`/api/models/${id}`)).status()).toBe(200);
      await page.unroute('**/api/catalog/materials');
      const next = page.waitForResponse(isUpload);
      await page.getByLabel('Wybierz modele', { exact: true }).setInputFiles(cube);
      expect((await next).status()).toBe(201);
    } else await page.unroute('**/api/catalog/materials');
    await gone(page, id);
  });
}
