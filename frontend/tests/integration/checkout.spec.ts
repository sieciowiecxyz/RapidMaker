import { expect, test } from '@playwright/test';
import { resolve } from 'node:path';
import { fillStripeCard } from './helpers';

test.use({ screenshot: 'off', trace: 'off' });
test('real automatic pickup checkout stays on the site and pays the server amount', async ({
  page,
}) => {
  test.skip(process.env.RAPIDMAKER_SANDBOX !== '1', 'Requires --sandbox; Stripe test card only');
  test.setTimeout(120_000);
  await page.goto('/wycena', { waitUntil: 'domcontentloaded' });
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
  await page.getByLabel('Imię i nazwisko', { exact: true }).fill('Test Rapidmaker');
  await page.getByLabel('Email', { exact: true }).fill('void@sieciowiec.xyz');
  await page.getByLabel('Telefon', { exact: true }).fill('500600700');
  await page.getByRole('radio', { name: /Odbiór osobisty/ }).check();
  await expect(page.getByLabel('Ulica, numer domu i mieszkania', { exact: true })).toHaveCount(0);
  const summary = page.getByRole('button', { name: 'Podsumowanie →', exact: true });
  await expect(summary).toBeEnabled();
  await summary.click();
  await expect(
    page.getByRole('heading', { name: 'Sprawdź zamówienie.', exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Przejdź do płatności →', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Dokończ płatność.', exact: true })).toBeVisible();
  await fillStripeCard(page);
  await page.getByRole('button', { name: /^Zapłać .*→$/ }).click();
  await expect(page).toHaveURL(/localhost:18437\/zamowienie\?payment_intent=pi_/);
  const intent = new URL(page.url()).searchParams.get('payment_intent')!;
  await expect
    .poll(
      async () => (await (await page.request.get(`/api/checkout/intent/${intent}`)).json()).status,
      { timeout: 30_000 },
    )
    .toBe('paid');
});
