import { expect, test } from '@playwright/test';
import { resolve } from 'node:path';

for (const width of [1440, 390]) {
  test(`background follows shrinking checkout content at ${width}px`, async ({ page }, info) => {
    await page.setViewportSize({ width, height: 900 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.name));
    // Layout regression only: deterministic API responses, no provider operations.
    await page.route('**/api/**', async (route) => {
      const path = new URL(route.request().url()).pathname;
      let data: unknown;
      if (path === '/api/auth/me') return route.fulfill({ status: 401, json: {} });
      if (path === '/api/catalog/materials')
        data = {
          materials: [
            {
              id: 'GFA00',
              name: 'PLA',
              available: true,
              colors: [{ code: '10101', name: 'Black', rgba: ['#000000FF'], in_stock: false }],
            },
          ],
        };
      else if (path === '/api/models')
        data = {
          id: '00000000-0000-4000-8000-000000000002',
          filename: 'cube_20mm.stl',
          dimensions_mm: [20, 20, 20],
          weight_g: null,
          weight_source: 'slicer',
        };
      else if (path === '/api/quotes/estimate')
        data = {
          id: 'layout-quote',
          status: 'succeeded',
          result: {
            id: 'layout-quote',
            expires_at: Math.floor(Date.now() / 1000) + 600,
          },
        };
      else if (path === '/api/checkout/quote') {
        const part = route.request().postDataJSON().parts[0];
        data = {
          id: 'layout-cart',
          expires_at: Math.floor(Date.now() / 1000) + 600,
          packing: { status: 'estimated', parcels: [], packaging_grosze: 0 },
          offers: [],
          print_grosze: 3650,
          setup_grosze: 1200,
          packaging_grosze: 0,
          print_lines: [{ model_id: part.id, quantity: 1, unit_grosze: 2450, total_grosze: 2450 }],
        };
      } else throw new Error(`Unexpected layout fixture route: ${path}`);
      return route.fulfill({ status: path === '/api/models' ? 201 : 200, json: data });
    });
    await page.goto('/wycena');
    await page.evaluate(() => document.fonts.ready);
    await page
      .getByLabel('Dodaj modele STL, STEP lub 3MF')
      .setInputFiles(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
    await page.locator('input[name=material][value=pla]').check();
    await page.locator('input[name=quality][value=standard]').check();
    await page.locator('input[name=strength][value=standard]').check();
    await expect(page.locator('.quote-contact')).toBeEnabled();
    const landingHeight = await page.evaluate(() => document.body.scrollHeight);
    await page.locator('.quote-contact').click();
    await expect(page.locator('#order-title')).toBeVisible();
    await expect(page.locator('#main')).toBeHidden();
    await expect
      .poll(() =>
        page.evaluate(() => {
          const orderBottom =
            document.querySelector('.order-page')!.getBoundingClientRect().bottom + scrollY;
          return document.body.scrollHeight - Math.max(innerHeight, orderBottom);
        }),
      )
      .toBeLessThanOrEqual(4);
    expect(await page.evaluate(() => document.body.scrollHeight)).toBeLessThan(landingHeight);
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
      width,
    );
    await page.screenshot({ path: info.outputPath('checkout.png'), fullPage: true });
    await page.getByRole('button', { name: 'Wróć do konfiguracji', exact: true }).first().click();
    await expect(page.locator('#main')).toBeVisible();
    await expect
      .poll(() => page.evaluate(() => document.body.scrollHeight))
      .toBeGreaterThanOrEqual(landingHeight - 4);
    await page.getByRole('button', { name: 'Zmień na jasny motyw' }).click();
    await expect(page.locator('.flow-background')).toBeHidden();
    expect(errors).toEqual([]);
  });
}
