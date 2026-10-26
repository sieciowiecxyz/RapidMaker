import { expect, test, type Page } from '@playwright/test';
import { resolve } from 'node:path';

const modelId = '00000000-0000-4000-8000-000000000002';
async function configure(page: Page, configured = true) {
  const estimates: Record<string, unknown>[] = [];
  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === '/api/auth/me') return route.fulfill({ status: 401, json: {} });
    if (path === '/api/catalog/materials')
      return route.fulfill({
        json: {
          materials: [
            {
              id: 'GFG02',
              name: 'PETG',
              available: configured,
              colors: [{ code: '33102', name: 'Black', rgba: ['#000000FF'], in_stock: false }],
            },
          ],
        },
      });
    if (path === '/api/models')
      return route.fulfill({
        status: 201,
        json: {
          id: modelId,
          filename: 'cube_20mm.stl',
          dimensions_mm: [20, 20, 20],
          weight_g: null,
          weight_source: 'slicer',
        },
      });
    if (path === '/api/quotes/estimate') {
      estimates.push(route.request().postDataJSON());
      return configured
        ? route.fulfill({
            status: 202,
            json: {
              id: 'measured-quote',
              status: 'succeeded',
              result: { id: 'measured-quote', expires_at: Math.floor(Date.now() / 1000) + 600 },
            },
          })
        : route.fulfill({
            status: 422,
            json: {
              code: 'manual_quote_required',
              message: 'Materiał wymaga zatwierdzenia bezpiecznego profilu i jawnej ceny zakupu.',
            },
          });
    }
    if (path === '/api/checkout/quote') {
      const part = route.request().postDataJSON().parts[0];
      return route.fulfill({
        json: {
          id: 'cart-quote',
          expires_at: Math.floor(Date.now() / 1000) + 600,
          packing: { status: 'estimated', parcels: [], packaging_grosze: 0 },
          offers: [],
          print_grosze: 3650,
          print_lines: [{ model_id: part.id, quantity: 1, unit_grosze: 2450, total_grosze: 2450 }],
          setup_grosze: 1200,
          packaging_grosze: 0,
        },
      });
    }
    throw new Error('Unexpected API: ' + path);
  });
  await page.goto('/wycena');
  await page
    .getByLabel('Dodaj modele STL, STEP lub 3MF')
    .setInputFiles(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
  await page.locator('input[name=material][value=petg]').check();
  await page.locator('input[name=quality][value=standard]').check();
  await page.locator('input[name=strength][value=standard]').check();
  return estimates;
}

test.use({ reducedMotion: 'reduce' });
test('a cross-family shade shows the measured material price but remains an unmodified manual request', async ({
  page,
}) => {
  const estimates = await configure(page);
  await page.locator('input[name=color][value=custom]').check();
  await page.getByRole('button', { name: 'PLA-CF', exact: true }).click();
  await page.getByRole('radio', { name: 'Matcha Green', exact: true }).check();
  const result = page.locator('.quote-result');
  await expect(result.getByText('Szacowana cena wydruku', { exact: true })).toBeVisible();
  await expect(result.locator('.price')).toContainText('36,50');
  await expect(result).toContainText('PETG wymaga potwierdzenia');
  expect(estimates.length).toBeGreaterThan(0);
  expect(estimates.every((r) => r.material_id === 'GFG02' && r.color_code === '33102')).toBe(true);
  await page.getByRole('button', { name: 'Zapytaj o wycenę', exact: true }).click();
  await expect(page.locator('#order-title')).toBeVisible();
  const model = await page.evaluate(
    () => JSON.parse(localStorage.getItem('rapidmaker.frontend.demo.v1')!).cart.models[0],
  );
  expect(model.options.materialId).toBe('GFG02');
  expect(model.options.color).toBe('');
  expect(model.colorPreference).toContain('Matcha Green');
  await expect(page.locator('.order-total')).toContainText('Szacowana cena wydruku');
  await expect(page.locator('.order-total')).toContainText('36,50');
  await expect(
    page.getByRole('button', { name: 'Przejdź do indywidualnej wyceny', exact: true }),
  ).toBeVisible();
});

test('missing operator configuration preserves the backend reason instead of hiding it behind generic manual text', async ({
  page,
}) => {
  const estimates = await configure(page, false);
  await expect(page.locator('.quote-result')).toContainText(
    'Materiał wymaga zatwierdzenia bezpiecznego profilu i jawnej ceny zakupu.',
  );
  expect(estimates.length).toBeGreaterThan(0);
  await expect(page.locator('.quote-result .price')).toHaveCount(0);
});
