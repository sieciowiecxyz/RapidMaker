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
  const status = await (await page.request.get(`/api/checkout/intent/${intent}`)).json();
  expect(status).toMatchObject({
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
  const orderId: string = status.order_id;
  expect(orderId).toBeTruthy();
  const localReceiptFree = (inject: boolean) =>
    page.evaluate(
      ({ inject, orderId }) => {
        const key = 'rapidmaker.frontend.demo.v1';
        const saved = localStorage.getItem(key);
        if (!saved) throw new Error('Expected persisted checkout cart');
        const state: { cart: { models: unknown[] }; orders: Record<string, unknown>[] } =
          JSON.parse(saved);
        const orders = JSON.stringify(state.orders);
        const clean =
          !state.cart.models.length &&
          !state.orders.some((order) => order.sandboxPayment) &&
          !orders.includes(orderId) &&
          !/LEGACY-RECEIPT-CONTACT|void@sieciowiec.xyz|Test Rapidmaker|500600700/.test(orders);
        if (inject) {
          state.orders.push({
            sandboxPayment: { sessionId: 'legacy' },
            contact: { name: 'LEGACY-RECEIPT-CONTACT' },
          });
          localStorage.setItem(key, JSON.stringify(state));
          sessionStorage.removeItem('rapidmaker.last-checkout');
        }
        return clean;
      },
      { inject, orderId },
    );
  expect(await localReceiptFree(true)).toBe(true);
  const ownedOrder = page.waitForResponse(
    (r) => r.url().endsWith(`/api/account/orders/${orderId}`) && r.request().method() === 'GET',
  );
  await page.reload();
  expect((await ownedOrder).status()).toBe(200);
  await expect(
    page.getByRole('heading', { name: 'Płatność potwierdzona.', exact: true }),
  ).toBeVisible();
  await expect(page.locator('.payment-result')).toContainText(
    `Zamówienie RM-${orderId.slice(0, 8).toUpperCase()}`,
  );
  await expect(
    page.getByRole('link', { name: 'Utwórz konto i śledź zamówienie →', exact: true }),
  ).toHaveAttribute('href', '/logowanie?register=1&email=void%40sieciowiec.xyz');
  for (const cached of [
    '{',
    JSON.stringify({ sessionId: intent, items: null, delivery: 'invalid-cache' }),
    JSON.stringify({
      sessionId: intent,
      items: [{ name: 'invalid-cache', quantity: 0 }],
      delivery: 'invalid-cache',
    }),
  ]) {
    await page.evaluate(
      (value) => sessionStorage.setItem('rapidmaker.last-checkout', value),
      cached,
    );
    await page.reload();
    await expect(
      page.getByRole('heading', { name: 'Płatność potwierdzona.', exact: true }),
    ).toBeVisible();
    await expect(page.locator('.payment-result')).toContainText(
      `Zamówienie RM-${orderId.slice(0, 8).toUpperCase()}`,
    );
    await expect(page.getByText('invalid-cache', { exact: true })).toHaveCount(0);
  }
  // A real verified payment remains paid when private order details are unavailable.
  await page.route(`**/api/account/orders/${orderId}`, (route) =>
    route.fulfill({ status: 404, json: { message: 'Nie znaleziono zamówienia.' } }),
  );
  await page.reload();
  await expect(
    page.getByRole('heading', { name: 'Płatność potwierdzona.', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Nie udało się sprawdzić płatności', exact: true }),
  ).toHaveCount(0);
  await page.unroute(`**/api/account/orders/${orderId}`);
  expect(await localReceiptFree(false)).toBe(true);
});
