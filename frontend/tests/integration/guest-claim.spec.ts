import { expect, test, type Page } from './fixtures';
import { resolve } from 'node:path';
import { accounts, fillStripeCard, login, totp } from './helpers';
import { configureCheckout } from './checkout-helpers';

test.use({ screenshot: 'off', trace: 'off' });
test('guest payment follows the receipt login into durable exclusive account ownership', async ({
  page,
  context,
  browser,
}) => {
  test.skip(process.env.RAPIDMAKER_SANDBOX !== '1', 'Requires --sandbox; Stripe test only');
  test.setTimeout(240_000);
  const fixture = accounts();
  const client = fixture.find((a) => a.email === 'void@sieciowiec.xyz')!;
  test.skip(!client.totp_uri, 'Requires --client-totp; no additional email delivery');
  type Quote = {
    id: string;
    print_grosze: number;
    packaging_grosze: number;
    offers: { id: string; price_grosze: number }[];
  };
  const quotes: Promise<Quote>[] = [];
  const claims: Promise<number>[] = [];
  page.on('response', (r) => {
    if (r.url().endsWith('/api/checkout/quote') && r.ok()) quotes.push(r.json());
    if (r.url().endsWith('/api/auth/claim-guest') && r.request().method() === 'POST' && r.ok())
      claims.push(r.json().then((body) => body.resources_claimed));
  });
  expect((await page.request.get('/api/auth/me')).status()).toBe(401);
  await configureCheckout(page);
  const draft = await page.evaluate(() =>
    JSON.parse(localStorage.getItem('rapidmaker.storefront.draft.v1')!),
  );
  const modelId: string = draft.models[0].analysis.id;
  expect(modelId).toBeTruthy();
  await page.getByRole('radio', { name: /Odbiór osobisty/ }).check();
  await page.getByRole('button', { name: 'Podsumowanie →', exact: true }).click();
  await page.getByRole('button', { name: 'Przejdź do płatności →', exact: true }).click();
  await fillStripeCard(page);
  const created = page.waitForResponse(
    (r) => r.url().endsWith('/api/checkout/intent') && r.request().method() === 'POST',
  );
  await page.getByRole('button', { name: /^Zamawiam i płacę .*→$/ }).click();
  const intentResponse = await created;
  expect(intentResponse.status()).toBe(200);
  const input = intentResponse.request().postDataJSON();
  const quote = (await Promise.all(quotes)).find((q) => q.id === input.quote_id)!;
  expect(quote).toBeDefined();
  const offer = quote.offers.find((o) => o.id === input.offer_id)!;
  expect(offer).toBeDefined();
  const amount = quote.print_grosze + quote.packaging_grosze + offer.price_grosze;
  expect(Number.isSafeInteger(amount) && amount > 0).toBe(true);
  expect((await intentResponse.json()).total_grosze).toBe(amount);
  await expect(page).toHaveURL(/\/zamowienie\?payment_intent=pi_/);
  const intent = new URL(page.url()).searchParams.get('payment_intent')!;
  await expect
    .poll(
      async () => (await (await page.request.get(`/api/checkout/intent/${intent}`)).json()).status,
    )
    .toBe('paid');
  const status = await (await page.request.get(`/api/checkout/intent/${intent}`)).json();
  expect(status).toMatchObject({
    status: 'paid',
    total_grosze: amount,
    currency: 'pln',
    livemode: false,
  });
  const orderId: string = status.order_id;
  expect(orderId).toBeTruthy();
  const guestCookies = (await context.cookies()).filter((c) => c.name === 'rapidmaker_guest');
  expect(guestCookies.length).toBe(1);
  const oldGuest = await browser.newContext();
  const foreignGuest = await browser.newContext();
  const fresh = await browser.newContext();
  try {
    await oldGuest.addCookies(guestCookies);
    const oldPage = await oldGuest.newPage();
    const foreignPage = await foreignGuest.newPage();
    await foreignPage.goto('/wycena');
    const uploaded = foreignPage.waitForResponse(
      (r) => r.url().endsWith('/api/models') && r.request().method() === 'POST',
    );
    await foreignPage
      .getByLabel('Dodaj modele STL, STEP lub 3MF')
      .setInputFiles(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
    const foreignUpload = await uploaded;
    expect(foreignUpload.status()).toBe(201);
    // Chromium can evict the upload body during WebGL work; use the UI's persisted server ID.
    const foreignModelId = () =>
      foreignPage.evaluate(
        () =>
          JSON.parse(localStorage.getItem('rapidmaker.storefront.draft.v1') ?? 'null')?.models?.[0]
            ?.analysis?.id,
      );
    await expect.poll(foreignModelId).toBeTruthy();
    const foreignId: string = await foreignModelId();
    const orderPath = `/api/account/orders/${orderId}`;
    const modelPath = `/api/models/${modelId}`;
    const artifactPaths = [
      orderPath,
      `${orderPath}/files`,
      modelPath,
      `${modelPath}/source`,
      `${modelPath}/preview.glb`,
    ];
    for (const path of artifactPaths) expect((await page.request.get(path)).status()).toBe(200);
    const guestFiles = await (await page.request.get(`${orderPath}/files`)).json();
    const sliceQuoteId: string = guestFiles.files.find(
      (f: { model_id: string }) => f.model_id === modelId,
    ).quote_id;
    expect(sliceQuoteId).toBeTruthy();
    artifactPaths.push(guestFiles.files[0].source_url, guestFiles.files[0].preview_url);
    await expect(
      page.getByRole('heading', { name: 'Płatność potwierdzona.', exact: true }),
    ).toBeVisible();
    await page
      .getByRole('link', { name: 'Utwórz konto i śledź zamówienie →', exact: true })
      .click();
    await page.getByRole('button', { name: 'Mam już konto', exact: true }).click();
    await page.getByLabel('Adres email', { exact: true }).fill(client.email);
    await page.getByLabel('Hasło', { exact: true }).fill(client.password);
    await page.getByRole('button', { name: 'Zaloguj hasłem', exact: true }).click();
    await page.getByLabel('Kod z aplikacji', { exact: true }).fill(totp(client.totp_uri!));
    await page.getByRole('button', { name: 'Potwierdź i zaloguj', exact: true }).click();
    await expect(page).toHaveURL(/\/konto\/zamowienia$/);
    expect((await Promise.all(claims)).some((count) => count > 0)).toBe(true);
    const detailLink = page.getByRole('link', {
      name: `Szczegóły zamówienia RM-${orderId.slice(0, 8).toUpperCase()}`,
      exact: true,
    });
    await expect(detailLink).toHaveCount(1);
    await detailLink.click();
    await expect(page).toHaveURL(new RegExp(`/konto/zamowienia/${orderId}$`));
    await expect(page.getByText('cube_20mm.stl', { exact: true }).first()).toBeVisible();
    const verifyOwned = async (target: Page) => {
      const list = await (await target.request.get('/api/account/orders')).json();
      expect(list.orders.filter((o: { id: string }) => o.id === orderId)).toHaveLength(1);
      const order = await (await target.request.get(orderPath)).json();
      expect(order).toMatchObject({ id: orderId, amount_grosze: amount, payment_state: 'paid' });
      expect(order.lines.some((line: { model_id: string }) => line.model_id === modelId)).toBe(
        true,
      );
      for (const path of artifactPaths) expect((await target.request.get(path)).status()).toBe(200);
      const files = await (await target.request.get(`${orderPath}/files`)).json();
      const file = files.files.find((f: { model_id: string }) => f.model_id === modelId);
      expect(file).toMatchObject({
        available: true,
        filename: 'cube_20mm.stl',
        quote_id: sliceQuoteId,
      });
      expect((await target.request.get(file.source_url)).status()).toBe(200);
      const preview = await target.request.get(file.preview_url);
      expect(preview.status()).toBe(200);
      expect((await preview.body()).subarray(0, 4).toString()).toBe('glTF');
    };
    await verifyOwned(page);
    const claimAgain = async (target: Page) => {
      const me = await (await target.request.get('/api/auth/me')).json();
      const response = await target.request.post('/api/auth/claim-guest', {
        headers: { Origin: 'http://localhost:18437', 'X-CSRF-Token': me.csrf_token },
        data: {},
      });
      expect(response.status()).toBe(200);
      expect((await response.json()).resources_claimed).toBe(0);
    };
    await claimAgain(page);
    await page.reload();
    await verifyOwned(page);
    // Both still carry valid guest cookies: scope succeeds, ownership lookup must return 404.
    for (const path of artifactPaths)
      expect((await oldPage.request.get(new URL(path, page.url()).href)).status()).toBe(404);
    expect((await foreignPage.request.get(`/api/models/${foreignId}`)).status()).toBe(200);
    expect((await page.request.get(`/api/models/${foreignId}`)).status()).toBe(404);
    await login(
      oldPage,
      fixture.find((a) => a.email === 'browser-other@example.invalid')!,
    );
    await claimAgain(oldPage);
    for (const path of artifactPaths) expect((await oldPage.request.get(path)).status()).toBe(404);
    const me = await (await page.request.get('/api/auth/me')).json();
    expect(
      (
        await page.request.post('/api/auth/logout', {
          headers: { Origin: 'http://localhost:18437', 'X-CSRF-Token': me.csrf_token },
          data: {},
        })
      ).status(),
    ).toBe(200);
    expect((await page.request.get(orderPath)).status()).toBe(404);
    expect((await fresh.cookies()).length).toBe(0);
    const freshPage = await fresh.newPage();
    await login(freshPage, client);
    await verifyOwned(freshPage);
    expect((await foreignPage.request.get(`/api/models/${foreignId}`)).status()).toBe(200);
  } finally {
    await Promise.all([oldGuest.close(), foreignGuest.close(), fresh.close()]);
  }
});
