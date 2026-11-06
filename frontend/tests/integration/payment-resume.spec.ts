import { expect, test } from './fixtures';
import { resolve } from 'node:path';
import { configureCheckout } from './checkout-helpers';
import { fillStripeCard, stripeEvidence } from './helpers';
import type { CheckoutQuote } from '../../src/lib/types';

test.use({ screenshot: 'off', trace: 'off', video: 'off' });

test('declined guest payment resumes after leaving with the same intent, order and amount', async ({
  page,
  browser,
}) => {
  test.skip(process.env.RAPIDMAKER_SANDBOX !== '1', 'Requires Stripe test sandbox');
  test.setTimeout(180_000);
  const quotes: Promise<CheckoutQuote>[] = [];
  const resumes: Promise<{ id: string; order: string; amount: number }>[] = [];
  const creations: string[] = [];
  page.on('request', (request) => {
    const path = new URL(request.url()).pathname;
    if (
      request.method() === 'POST' &&
      (/^\/api\/checkout\/(intent|session|drafts)$/.test(path) ||
        /^\/api\/checkout\/drafts\/[^/]+\/payment$/.test(path))
    )
      creations.push(path);
  });
  page.on('response', (response) => {
    if (response.url().endsWith('/api/checkout/quote') && response.ok())
      quotes.push(response.json());
    if (/\/api\/checkout\/intent\/[^/]+\/resume$/.test(response.url()) && response.ok())
      resumes.push(
        response.json().then((body) => ({
          id: body.payment_intent_id,
          order: body.order_id,
          amount: body.total_grosze,
        })),
      );
  });
  expect((await page.request.get('/api/auth/me')).status()).toBe(401);
  await configureCheckout(page);
  await page.getByRole('radio', { name: /Odbiór osobisty/ }).check();
  await page.getByRole('button', { name: 'Podsumowanie →', exact: true }).click();
  await page.getByRole('button', { name: 'Przejdź do płatności →', exact: true }).click();
  await fillStripeCard(page, '4000000000000002');
  const created = page.waitForResponse(
    (r) => r.url().endsWith('/api/checkout/intent') && r.request().method() === 'POST',
  );
  await page.getByRole('button', { name: /^Zamawiam i płacę/ }).click();
  const response = await created;
  expect(response.status()).toBe(200);
  // Keep only safe correlation fields; never assert/attach the client secret or full body.
  const {
    payment_intent_id: intent,
    order_id: orderId,
    total_grosze: amount,
  } = await response.json();
  expect(intent).toMatch(/^pi_/);
  expect(orderId).toBeTruthy();
  const input = response.request().postDataJSON();
  const quote = (await Promise.all(quotes)).find((q) => q.id === input.quote_id)!;
  expect(quote).toBeDefined();
  const offer = quote.offers.find((o) => o.id === input.offer_id)!;
  expect(offer).toBeDefined();
  expect(amount).toBe(quote.print_grosze + quote.packaging_grosze + offer.price_grosze);
  expect(Number.isSafeInteger(amount) && amount > 0).toBe(true);
  await expect(page.getByRole('alert').first()).toContainText(/odrzucon|declin/i, {
    timeout: 30_000,
  });
  const statusPath = `/api/checkout/intent/${intent}`;
  const before = await (await page.request.get(statusPath)).json();
  expect(before).toMatchObject({
    status: 'unpaid',
    checkout_status: 'requires_payment_method',
    order_id: orderId,
    total_grosze: amount,
    livemode: false,
  });
  expect(creations).toEqual(['/api/checkout/intent']);

  const foreign = await browser.newContext();
  try {
    const stranger = await foreign.newPage();
    await stranger.goto('/wycena');
    const uploaded = stranger.waitForResponse(
      (r) => r.url().endsWith('/api/models') && r.request().method() === 'POST',
    );
    await stranger
      .getByLabel('Dodaj modele STL, STEP lub 3MF')
      .setInputFiles(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
    expect((await uploaded).status()).toBe(201);
    expect((await foreign.cookies()).some((c) => c.name === 'rapidmaker_guest')).toBe(true);
    // A valid different guest reaches the ownership lookup, which hides all references.
    for (const path of [statusPath, `${statusPath}/resume`, `/api/account/orders/${orderId}`]) {
      const denied = await stranger.request.get(path);
      expect(denied.status()).toBe(404);
      const body = await denied.json();
      expect(body.client_secret).toBeUndefined();
      expect(body.payment_intent_id).toBeUndefined();
      expect(body.order_id).toBeUndefined();
    }
    // Claim requires an authenticated account; a payment reference grants no authority.
    expect(
      (
        await stranger.request.post('/api/auth/claim-guest', {
          headers: { Origin: 'http://localhost:18437' },
          data: {},
        })
      ).status(),
    ).toBe(401);
  } finally {
    await foreign.close();
  }
  const unknown = await page.request.get(`${statusPath}_tampered`);
  expect(unknown.status()).toBe(404);
  expect((await unknown.json()).client_secret).toBeUndefined();

  // A real navigation destroys the original Stripe form and its in-memory intent.
  await page.goto('/wycena');
  await page.goto(`/zamowienie?payment_intent=${intent}`);
  await expect(
    page.getByRole('heading', { name: 'Płatność jeszcze przed Tobą.', exact: true }),
  ).toBeVisible();
  const retry = page.getByRole('region', { name: 'Dokończ płatność Stripe', exact: true });
  await expect(retry, 'Return page must mount RetryPayment for the declined intent').toBeVisible();
  await expect(retry.getByRole('button', { name: /^Zamawiam i płacę/ })).toBeEnabled({
    timeout: 30_000,
  });
  expect(await Promise.all(resumes)).toEqual([{ id: intent, order: orderId, amount }]);
  await fillStripeCard(page);
  await retry.getByRole('button', { name: /^Zamawiam i płacę/ }).click();
  await expect(
    page.getByRole('heading', { name: 'Płatność potwierdzona.', exact: true }),
  ).toBeVisible({ timeout: 35_000 });
  for (let repeat = 0; repeat < 2; repeat++) {
    const paid = await page.request.get(statusPath);
    expect(paid.status()).toBe(200);
    expect(await paid.json()).toMatchObject({
      status: 'paid',
      order_id: orderId,
      total_grosze: amount,
      quote_id: quote.id,
      currency: 'pln',
      livemode: false,
    });
    const receipt = await page.request.get(`/api/account/orders/${orderId}`);
    expect(receipt.status()).toBe(200);
    const order = await receipt.json();
    expect(order).toMatchObject({ id: orderId, amount_grosze: amount, payment_state: 'paid' });
    expect(order.paid_at).not.toBeNull();
    expect((await page.request.get(`${statusPath}/resume`)).status()).toBe(409);
    await page.reload();
    await expect(
      page.getByRole('heading', { name: 'Płatność potwierdzona.', exact: true }),
    ).toBeVisible();
    await expect(page.getByRole('button', { name: /^Zamawiam i płacę/ })).toHaveCount(0);
    await expect(retry).toHaveCount(0);
  }
  expect(creations).toEqual(['/api/checkout/intent']);
  expect(await Promise.all(resumes)).toEqual([{ id: intent, order: orderId, amount }]);
  // The compatibility endpoint returns the durable attempt ID as order_id.
  const provider = stripeEvidence({
    payment_intent_id: intent,
    attempt_id: orderId,
    order_id: orderId,
  });
  expect(provider.status).toBe('succeeded');
  expect(provider.currency).toBe('pln');
  expect(provider.amount).toBe(amount);
  expect(provider.amount_received).toBe(amount);
  expect(provider.refunds).toHaveLength(0);
});
