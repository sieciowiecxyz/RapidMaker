import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const fixture = (name: string) => resolve('../backend/crates/geometry/tests/fixtures', name);

test('real STL, STEP and 3MF upload produces private GLB without selecting a color', async ({
  page,
  browser,
}) => {
  await page.goto('/wycena', { waitUntil: 'domcontentloaded' });
  await expect(page.locator('.header')).toHaveCSS('display', 'flex');
  await expect(page.locator('.hero')).toHaveCSS('display', 'grid');
  await expect(page.locator('input[name=material]')).toHaveCount(3);
  const outsider = await browser.newContext();
  try {
    for (const name of ['cube_20mm.stl', 'step_block.step', 'simple_model.3mf']) {
      const uploaded = page.waitForResponse(
        (r) => r.url().endsWith('/api/models') && r.request().method() === 'POST',
      );
      await page.getByLabel('Dodaj modele STL, STEP lub 3MF').setInputFiles(fixture(name));
      const response = await uploaded;
      expect(response.status(), name).toBe(201);
      // Read the server ID saved by the real UI; Chromium may evict response bodies after WebGL work.
      await expect
        .poll(() =>
          page.evaluate((filename) => {
            const draft = JSON.parse(
              localStorage.getItem('rapidmaker.storefront.draft.v1') ?? 'null',
            );
            return draft?.models?.find((m: { name: string }) => m.name === filename)?.analysis?.id;
          }, name),
        )
        .toBeTruthy();
      const id = await page.evaluate(
        (filename) =>
          JSON.parse(localStorage.getItem('rapidmaker.storefront.draft.v1')!).models.find(
            (m: { name: string }) => m.name === filename,
          ).analysis.id,
        name,
      );
      const metadata = await page.request.get(`/api/models/${id}`);
      expect(metadata.status()).toBe(200);
      const model = await metadata.json();
      expect(model.filename).toBe(name);
      expect(model.dimensions_mm.every((v: number) => Number.isFinite(v) && v > 0)).toBe(true);
      const preview = await page.request.get(model.preview_url);
      expect(preview.status()).toBe(200);
      expect((await preview.body()).subarray(0, 4).toString()).toBe('glTF');
      expect(
        (await outsider.request.get(new URL(model.preview_url, page.url()).href)).status(),
      ).toBe(401);
      await expect(
        page.getByRole('img', {
          name: `Model z pliku ${name}. Obróć przeciągając myszą. Kolor: Biały podgląd.`,
        }),
      ).toBeVisible();
      await expect(page.getByText('Podgląd 3D jest niedostępny na tym urządzeniu.')).toHaveCount(0);
    }
    await page.screenshot({
      path: 'tests/artifacts/integration/configure-desktop.png',
      fullPage: true,
    });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.screenshot({
      path: 'tests/artifacts/integration/configure-mobile.png',
      fullPage: true,
    });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
      true,
    );
    await page.getByRole('button', { name: 'Usuń step_block.step', exact: true }).click();
    await expect(
      page.getByRole('button', { name: 'Konfiguruj step_block.step', exact: true }),
    ).toHaveCount(0);
    await page.reload({ waitUntil: 'domcontentloaded' });
    await expect(
      page.getByRole('button', { name: 'Konfiguruj cube_20mm.stl', exact: true }),
    ).toBeVisible();
    await page.getByRole('button', { name: 'Konfiguruj cube_20mm.stl', exact: true }).click();
    await expect(
      page.getByRole('img', { name: /Model z pliku cube_20mm.stl.*Biały podgląd/ }),
    ).toBeVisible();
    await page
      .getByLabel('Dodaj modele STL, STEP lub 3MF')
      .setInputFiles(fixture('step_block.step'));
    await expect(
      page.getByRole('img', { name: /Model z pliku step_block.step.*Biały podgląd/ }),
    ).toBeVisible();
  } finally {
    await outsider.close();
  }
});

test('real upload rejects malformed meshes and external/archive paths', async ({ request }) => {
  for (const name of [
    'invalid.stl',
    'nan_inf_coordinates.stl',
    'zip_path_traversal.3mf',
    'zip_external_relationship.3mf',
  ]) {
    const response = await request.post('/api/models', {
      headers: {
        'Content-Type': 'application/octet-stream',
        Origin: 'http://localhost:18437',
        'x-model-filename': encodeURIComponent(name),
      },
      data: readFileSync(fixture(name)),
    });
    expect([400, 422]).toContain(response.status());
    expect(await response.text()).not.toContain('AUTH_CODE_KEY');
  }
});

test('abandoned upload is physically removed after expiry, including mesh and GLB', async ({
  request,
}) => {
  const { execFileSync } = await import('node:child_process');
  const { existsSync } = await import('node:fs');
  const database = process.env.RAPIDMAKER_TEST_DATABASE ?? '';
  const root = process.env.RAPIDMAKER_TEST_MODEL_ROOT ?? '';
  expect(database).toMatch(/^rapidmaker_test_browser_[a-f0-9]+$/);
  expect(root).toMatch(/^\/tmp\/rapidmaker-browser-[^/]+\/models$/);
  const upload = () =>
    request.post('/api/models', {
      headers: {
        'Content-Type': 'application/octet-stream',
        Origin: 'http://localhost:18437',
        'x-model-filename': 'cube_20mm.stl',
      },
      data: readFileSync(fixture('cube_20mm.stl')),
    });
  const response = await upload();
  expect(response.status()).toBe(201);
  const model = await response.json();
  expect(model.id).toMatch(/^[a-f0-9-]{36}$/);
  expect(model.expires_at).toBeGreaterThan(Date.now() / 1000 + 86300);
  const directory = resolve(root, model.id);
  for (const file of ['model.stl', 'canonical.stl', 'preview.glb'])
    expect(existsSync(resolve(directory, file))).toBe(true);
  // Controlled time only in the disposable database, never the developer's files.
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
      '-c',
      `UPDATE private_models SET expires_at=extract(epoch from now())::bigint-1 WHERE id='${model.id}'`,
    ],
    { stdio: 'pipe' },
  );
  expect((await upload()).status()).toBe(201); // Upload admission runs cleanup as well as the worker.
  expect(existsSync(directory)).toBe(false);
  expect((await request.get(model.preview_url)).status()).toBe(404);
});

test('multiple clients receive isolated results while costly conversions are bounded', async ({
  playwright,
}) => {
  const clients = await Promise.all(
    Array.from({ length: 4 }, () =>
      playwright.request.newContext({ baseURL: 'http://localhost:18437' }),
    ),
  );
  const upload = (client: (typeof clients)[number]) =>
    client.post('/api/models', {
      headers: {
        'Content-Type': 'application/octet-stream',
        Origin: 'http://localhost:18437',
        'x-model-filename': 'step_cylinder.step',
      },
      data: readFileSync(fixture('step_cylinder.step')),
    });
  try {
    const pending = clients.map(upload);
    const started = Date.now();
    expect((await clients[0].get('/api/catalog/materials')).status()).toBe(200);
    expect(Date.now() - started).toBeLessThan(2000);
    const first = await Promise.all(pending);
    const ids: string[] = [];
    for (let i = 0; i < clients.length; i++) {
      expect([201, 429]).toContain(first[i].status());
      const result = first[i].status() === 429 ? await upload(clients[i]) : first[i];
      expect(result.status()).toBe(201);
      const model = await result.json();
      ids.push(model.id);
      expect((await clients[i].get(model.preview_url)).status()).toBe(200);
    }
    expect(new Set(ids).size).toBe(clients.length);
    for (let i = 0; i < clients.length; i++)
      expect(
        (await clients[(i + 1) % clients.length].get(`/api/models/${ids[i]}/preview.glb`)).status(),
      ).toBe(404);
  } finally {
    await Promise.all(clients.map((client) => client.dispose()));
  }
});
