import { test, expect } from '@playwright/test';

test('public legal documents render safely and can be saved without an account', async ({
  page,
}) => {
  await page.route('**/api/auth/me', (route) => route.fulfill({ status: 401, json: {} }));
  await page.route('**/api/legal/documents', (route) =>
    route.fulfill({
      json: {
        version: '2026-10-25',
        production_days: 14,
        terms:
          '1. Sprzedawca\n\nPiotr Grudniak Rapid Maker\nKontakt: piotr@rapidmaker.pl\n\n2. Zamówienia\n\nWydruk <script> nie jest kodem strony.',
        privacy: '1. Twoje dane\n\nUsunięcie konta: piotr@rapidmaker.pl.',
      },
    }),
  );
  await page.goto('/regulaminy');
  await page.getByRole('link', { name: /Regulamin Zamówienia/ }).click();
  await expect(page.getByRole('heading', { name: '1. Sprzedawca' })).toBeVisible();
  await expect(page.getByText('Wydruk <script> nie jest kodem strony.')).toBeVisible();
  const saved = page.waitForEvent('download');
  await page.getByRole('button', { name: /Pobierz dokument/ }).click();
  expect((await saved).suggestedFilename()).toBe('rapidmaker-regulamin-2026-10-25.txt');
  await page.getByRole('link', { name: /Wszystkie dokumenty/ }).click();
  await page.getByRole('link', { name: /Polityka prywatności Dane/ }).click();
  await expect(page.getByRole('heading', { name: '1. Twoje dane' })).toBeVisible();
});

test('voluntary registration displays linked terms and records their version without a checkbox', async ({
  page,
}) => {
  await page.route('**/api/auth/me', (route) => route.fulfill({ status: 401, json: {} }));
  let submitted: unknown;
  await page.route('**/api/auth/start', (route) => {
    submitted = route.request().postDataJSON();
    return route.fulfill({ json: { challenge_id: 'legal-enrollment' } });
  });
  await page.goto('/logowanie?register=1');
  await expect(page.getByText('Tworząc konto, akceptujesz', { exact: false })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Regulamin', exact: true })).toHaveAttribute(
    'href',
    '/regulamin',
  );
  await expect(page.getByRole('checkbox')).toHaveCount(0);
  await page.getByLabel('Adres email').fill('client@example.invalid');
  await page.getByRole('button', { name: /Utwórz konto — potwierdź email/ }).click();
  await expect
    .poll(() => submitted)
    .toEqual({ email: 'client@example.invalid', legal_version: '2026-10-25' });
});
