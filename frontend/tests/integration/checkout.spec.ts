import { expect, test } from './fixtures';
import { configureCheckout } from './checkout-helpers';
import { fillStripeCard } from './helpers';

test.use({ screenshot: 'off', trace: 'off' });
test('real automatic pickup checkout stays on the site and pays the server amount', async ({
  page,
}) => {
  test.skip(process.env.RAPIDMAKER_SANDBOX !== '1', 'Requires --sandbox; Stripe test card only');
  test.setTimeout(120_000);
  type Quote = {
    id: string;
    print_grosze: number;
    packaging_grosze: number;
    offers: { id: string; price_grosze: number }[];
  };
  const quotes: Promise<Quote>[] = [];
  page.on('response', (response) => {
    if (response.url().endsWith('/api/checkout/quote') && response.ok())
      quotes.push(response.json());
  });
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
  const pay = page.getByRole('button', { name: /^Zamawiam i płacę .*→$/ });
  const label = await pay.innerText();
  const created = page.waitForResponse(
    (r) => r.url().endsWith('/api/checkout/intent') && r.request().method() === 'POST',
  );
  await pay.click();
  const intentResponse = await created;
  expect(intentResponse.status()).toBe(200);
  const input = intentResponse.request().postDataJSON();
  const quote = (await Promise.all(quotes)).find((q) => q.id === input.quote_id)!;
  expect(quote, 'Payment must reference an observed server quote').toBeDefined();
  const offer = quote.offers.find((o) => o.id === input.offer_id)!;
  expect(offer, 'Payment must use an offer from that quote').toBeDefined();
  const expected = quote.print_grosze + quote.packaging_grosze + offer.price_grosze;
  expect(Number.isSafeInteger(expected) && expected > 0).toBe(true);
  expect((await intentResponse.json()).total_grosze).toBe(expected);
  const displayed = new Intl.NumberFormat('pl-PL', { style: 'currency', currency: 'PLN' }).format(
    expected / 100,
  );
  expect(label.replace(/\s/g, '')).toBe(`Zamawiamipłacę${displayed.replace(/\s/g, '')}→`);
  await expect(page).toHaveURL(/localhost:18437\/zamowienie\?payment_intent=pi_/);
  const intent = new URL(page.url()).searchParams.get('payment_intent')!;
  await expect
    .poll(
      async () => (await (await page.request.get(`/api/checkout/intent/${intent}`)).json()).status,
      { timeout: 30_000 },
    )
    .toBe('paid');
  expect(await (await page.request.get(`/api/checkout/intent/${intent}`)).json()).toMatchObject({
    status: 'paid',
    total_grosze: expected,
    currency: 'pln',
    livemode: false,
  });
  await expect(
    page.getByRole('heading', { name: 'Płatność potwierdzona.', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('link', { name: 'Utwórz konto i śledź zamówienie →', exact: true }),
  ).toBeVisible();
});
