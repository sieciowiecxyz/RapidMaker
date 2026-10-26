import { expect, type Page } from './fixtures';
import { resolve } from 'node:path';

export async function configureCheckout(page: Page) {
  await page.goto('/wycena');
  await page
    .getByLabel('Dodaj modele STL, STEP lub 3MF')
    .setInputFiles(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
  await expect(page.getByRole('img', { name: /Model z pliku cube_20mm.stl/ })).toBeVisible();
  await page.locator('input[name=material][value=GFA00]').check();
  await page.locator('input[name=color]').first().check();
  await page.locator('input[name=quality][value=standard]').check();
  await page.locator('input[name=strength][value=standard]').check();
  await expect(page.getByRole('button', { name: 'Przejdź do dostawy', exact: true })).toBeEnabled({
    timeout: 90_000,
  });
  await page.getByRole('button', { name: 'Przejdź do dostawy', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Dokąd wysłać wydruk?', exact: true }),
  ).toBeVisible();
}

export async function fillStripeAddress(page: Page) {
  await page.locator('input[name="delivery"][value="courier"]').check();
  const fields: [string, string][] = [
    ['input[name="name"]', 'Test Rapidmaker'],
    ['input[name="addressLine1"]', 'Testowa 1'],
    ['input[name="postalCode"]', '05-500'],
    ['input[name="locality"]', 'Piaseczno'],
  ];
  for (const [selector, value] of fields) {
    let filled = false;
    await expect
      .poll(
        async () => {
          for (const frame of page.frames()) {
            if (!frame.url().startsWith('https://js.stripe.com/')) continue;
            const input = frame.locator(selector);
            if (await input.isVisible().catch(() => false)) {
              await input.fill(value);
              await input.press('Tab');
              filled = true;
              return true;
            }
          }
          return filled;
        },
        { timeout: 30_000, message: `Stripe address field ${selector} is available` },
      )
      .toBe(true);
  }
}
