import { expect, test } from '@playwright/test';

// Controlled display contract: the real checkout and API adapter render fixture amounts.
test('checkout multiplies unit prices only when they reproduce the exact line total', async ({
  page,
}) => {
  const lines = [
    { model_id: 'rounded', quantity: 3, unit_grosze: 21, total_grosze: 62 },
    { model_id: 'exact', quantity: 3, unit_grosze: 21, total_grosze: 63 },
    { model_id: 'single', quantity: 1, unit_grosze: 29, total_grosze: 29 },
  ];
  await page.addInitScript((lines) => {
    localStorage.setItem(
      'rapidmaker.frontend.demo.v1',
      JSON.stringify({
        orders: [],
        scenario: 'normal',
        revision: 1,
        cart: {
          delivery: 'pickup',
          postalCode: '',
          quote: null,
          models: lines.map((line) => ({
            id: line.model_id,
            name: `${line.model_id}.stl`,
            format: 'STL',
            units: 'mm',
            parts: 1,
            notes: '',
            strength: 'standard',
            options: {
              materialId: 'pla',
              color: 'black',
              quantity: line.quantity,
              quality: 'standard',
            },
            advanced: {
              wallCount: 2,
              infill: 15,
              pattern: 'grid',
              orientation: 'auto',
              supports: 'auto',
              finish: 'standard',
              fit: 'normal',
              insertCount: 0,
              turnaround: 'standard',
            },
            analysis: { id: line.model_id, dimensions_mm: [20, 20, 20] },
          })),
        },
      }),
    );
  }, lines);
  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === '/api/auth/me') return route.fulfill({ status: 401, json: {} });
    if (path === '/api/checkout/config') return route.fulfill({ json: { publishable_key: null } });
    if (path === '/api/catalog/materials')
      return route.fulfill({
        json: {
          materials: [
            {
              id: 'pla',
              name: 'PLA',
              available: true,
              colors: [{ code: 'black', name: 'Black', rgba: ['#000000FF'] }],
            },
          ],
        },
      });
    if (path === '/api/quotes/estimate')
      return route.fulfill({
        status: 202,
        json: {
          id: 'measured',
          status: 'succeeded',
          result: { id: 'measured', expires_at: Math.floor(Date.now() / 1000) + 600 },
        },
      });
    if (path === '/api/checkout/quote')
      return route.fulfill({
        json: {
          id: 'checkout',
          expires_at: Math.floor(Date.now() / 1000) + 600,
          print_lines: lines,
          print_grosze: 154,
          setup_grosze: 0,
          packaging_grosze: 0,
          packing: { status: 'estimated', parcels: [], packaging_grosze: 0 },
          offers: [],
        },
      });
    throw new Error('Unexpected API: ' + path);
  });
  await page.goto('/zamowienie');
  const summary = page.locator('aside.summary');
  for (const [name, detail, total] of [
    ['rounded', '3 szt.', '0,62'],
    ['exact', '3 szt. × 0,21 zł', '0,63 zł'],
    ['single', '1 szt. × 0,29 zł', '0,29 zł'],
  ]) {
    const row = summary.locator('.line').filter({ hasText: `${name}.stl` });
    await expect(row.locator('small')).toHaveText(detail);
    await expect(row.locator('strong')).toContainText(total);
  }
});
