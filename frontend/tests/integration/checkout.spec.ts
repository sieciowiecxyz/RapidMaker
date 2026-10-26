import { expect, test } from './fixtures';
import { configureCheckout } from './checkout-helpers';
import { fillStripeCard } from './helpers';

test.use({ screenshot: 'off', trace: 'off' });
test('real automatic pickup checkout stays on the site and pays the server amount', async ({
  page,
}) => {
  test.skip(process.env.RAPIDMAKER_SANDBOX !== '1', 'Requires --sandbox; Stripe test card only');
  test.setTimeout(120_000);
  await configureCheckout(page);
  await expect(page.locator('.embedded-contact')).toContainText('void@sieciowiec.xyz');
  await page.getByRole('radio', { name: /Odbiór osobisty/ }).check();
  await expect(page.getByLabel('Ulica, numer domu i mieszkania', { exact: true })).toHaveCount(0);
  const summary = page.getByRole('button', { name: 'Podsumowanie →', exact: true });
  await expect(summary).toBeEnabled();
  await summary.click();
  await expect(page.locator('.order-stage')).toContainText('Podsumowanie');
  await page.getByRole('button', { name: 'Przejdź do płatności →', exact: true }).click();
  await expect(page.locator('.order-stage')).toContainText('Płatność');
  await fillStripeCard(page);
  await page.getByRole('button', { name: /^Zamawiam i płacę .*→$/ }).click();
  await expect(page).toHaveURL(/localhost:18437\/zamowienie\?payment_intent=pi_/);
  const intent = new URL(page.url()).searchParams.get('payment_intent')!;
  await expect
    .poll(
      async () => (await (await page.request.get(`/api/checkout/intent/${intent}`)).json()).status,
      { timeout: 30_000 },
    )
    .toBe('paid');
  await expect(
    page.getByRole('heading', { name: 'Płatność potwierdzona.', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('link', { name: 'Utwórz konto i śledź zamówienie →', exact: true }),
  ).toBeVisible();
});
