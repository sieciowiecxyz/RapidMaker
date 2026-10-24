import { test, expect } from '@playwright/test';

test('operator color form submits valid RGBA instead of blocking native validation', async ({
  page,
}) => {
  // UI contract only: real server authorization is verified separately against PostgreSQL.
  await page.route('**/api/auth/me', (route) =>
    route.fulfill({
      json: {
        user: { id: 'operator-1', email: 'operator@example.invalid', role: 'admin' },
        admin_verified: true,
        csrf_token: 'csrf-color',
      },
    }),
  );
  await page.route('**/api/admin/materials', (route) =>
    route.fulfill({
      json: {
        materials: [
          {
            id: 'GFA00',
            name: 'Bambu PLA Basic',
            density_g_cm3: 1.26,
            cost_net_grosze_per_kg: 8000,
            production_enabled: true,
            source: { material: { p1p_04_profiles: ['Bambu PLA Basic @BBL P1P'] } },
            colors: [{ code: '10501', name: 'Green', rgba: ['#00AE42FF'], in_stock: false }],
          },
        ],
      },
    }),
  );
  await page.route('**/api/admin/materials/GFA00/colors/10501', (route) =>
    route.fulfill({ json: { saved: true } }),
  );
  await page.goto('/admin/materialy');
  await expect(page.getByRole('heading', { name: 'Materiały i kolory' })).toBeVisible();
  await page.locator('.materials-list > details > summary').click();
  const color = page.locator('.color-row');
  await color.getByLabel('W magazynie').check();
  const sent = page.waitForRequest(
    (request) => request.method() === 'PUT' && request.url().endsWith('/colors/10501'),
  );
  await color.getByRole('button', { name: 'Zapisz kolor', exact: true }).click();
  expect((await sent).headers()['x-csrf-token']).toBe('csrf-color');
  expect((await sent).postDataJSON()).toMatchObject({
    code: '10501',
    rgba: ['#00AE42FF'],
    in_stock: true,
  });
  await expect(color.getByRole('status')).toHaveText('Zapisano.');
});
