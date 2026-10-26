import { expect, test } from './fixtures';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const fixture = (name: string) => resolve('../backend/crates/geometry/tests/fixtures', name);

test('real configurator prices stock=false PLA with the slicer and invalidates quantity', async ({
  page,
}) => {
  await page.goto('/wycena', { waitUntil: 'domcontentloaded' });
  await page.getByLabel('Dodaj modele STL, STEP lub 3MF').setInputFiles(fixture('cube_20mm.stl'));
  await expect(page.getByRole('img', { name: /Model z pliku cube_20mm.stl/ })).toBeVisible();
  await page.locator('input[name=material][value=pla]').check();
  await page.locator('input[name=color]').first().check();
  await page.locator('input[name=quality][value=standard]').check();
  const quoteSent = page.waitForResponse(
    (r) => r.url().endsWith('/api/quotes/estimate') && r.request().method() === 'POST',
  );
  await page.locator('input[name=strength][value=standard]').check();
  expect((await quoteSent).status()).toBe(202);
  await expect(page.locator('.price')).toContainText(/\d.*zł/, { timeout: 90_000 });
  await expect(
    page.getByRole('button', { name: 'Przejdź do zamówienia', exact: true }),
  ).toBeEnabled();
  const previous = await page.locator('.price').innerText();
  await page.getByRole('spinbutton', { name: 'Liczba sztuk: cube_20mm.stl' }).fill('10');
  await expect(
    page.getByRole('button', { name: 'Przejdź do zamówienia', exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole('button', { name: 'Przejdź do zamówienia', exact: true }),
  ).toBeEnabled({
    timeout: 90_000,
  });
  expect(await page.locator('.price').innerText()).not.toBe(previous);
  await page.screenshot({
    path: 'tests/artifacts/integration/priced-configurator.png',
    fullPage: true,
  });
});

test('oversized server geometry requests manual handling before any slicer job', async ({
  request,
}) => {
  const upload = await request.post('/api/models', {
    headers: {
      'Content-Type': 'application/octet-stream',
      Origin: 'http://localhost:18437',
      'x-model-filename': 'huge_cube_1000mm.stl',
    },
    data: readFileSync(fixture('huge_cube_1000mm.stl')),
  });
  expect(upload.status()).toBe(201);
  const model = await upload.json();
  const catalog = await (await request.get('/api/catalog/materials')).json();
  const quote = await request.post('/api/quotes/estimate', {
    headers: { Origin: 'http://localhost:18437' },
    data: {
      model_id: model.id,
      material_id: 'GFA00',
      color_code: catalog.materials[0].colors[0].code,
      options: { quantity: 1, quality: 'standard', wall_loops: 3, infill_percent: 20 },
    },
  });
  expect(quote.status()).toBe(422);
  const error = await quote.json();
  expect(error).toMatchObject({ code: 'manual_quote_required', reason: 'size_limit' });
});
