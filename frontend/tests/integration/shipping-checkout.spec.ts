import { expect, test } from './fixtures';
import { configureCheckout, fillStripeAddress } from './checkout-helpers';
import { fillStripeCard } from './helpers';
import type { CheckoutQuote } from '../../src/lib/types';

test.use({ screenshot: 'off', trace: 'off' });

for (const method of ['locker', 'courier'] as const) {
  test(`real ${method} checkout keeps recipient data, validates invoice and pays the selected carrier`, async ({
    page,
  }, info) => {
    test.skip(
      process.env.RAPIDMAKER_SANDBOX !== '1',
      'Requires Stripe test and Furgonetka sandbox',
    );
    test.setTimeout(180_000);
    const quotes: Promise<CheckoutQuote>[] = [];
    page.on('response', (response) => {
      if (response.url().endsWith('/api/checkout/quote') && response.ok())
        quotes.push(response.json());
    });
    await configureCheckout(page, false);
    // Contact/invoice are edited once in the authored form, before the delivery step.
    await page.getByLabel('Imię i nazwisko', { exact: true }).fill('Test Rapidmaker');
    await page.getByLabel('E-mail', { exact: true }).fill('void@sieciowiec.xyz');
    await page.getByLabel('Telefon', { exact: true }).fill('500600700');
    await page.getByRole('checkbox', { name: 'Potrzebuję faktury na firmę', exact: true }).check();
    await page.getByLabel('Nazwa firmy', { exact: true }).fill('Firma testowa');
    const nip = page.getByLabel('NIP', { exact: true });
    await nip.fill('123');
    expect(await nip.evaluate((el: HTMLInputElement) => el.validity.valid)).toBe(false);
    await nip.fill('1234567883');
    await page
      .getByRole('checkbox', { name: 'Potrzebuję faktury na firmę', exact: true })
      .uncheck();
    await expect(nip).toHaveCount(0);
    await page.getByRole('button', { name: 'Wybierz dostawę i zapłać', exact: true }).click();
    if (method === 'locker') {
      await page.getByRole('radio', { name: /Paczkomat/ }).check();
      await expect(page.getByLabel('Ulica, numer domu i mieszkania', { exact: true })).toHaveCount(
        0,
      );
      await page.getByLabel('Kod pocztowy', { exact: true }).fill('05-500');
      await expect(page.getByLabel('Miejscowość', { exact: true })).toHaveCount(0);
      await expect(page.locator('.point-list button').first()).toBeVisible({ timeout: 35_000 });
      const first = await page.locator('.point-list button strong').first().innerText();
      await page.locator('.point-list button').first().click();
      await expect(page.locator('.selected-point-row')).toContainText(first);
      await page.getByRole('button', { name: 'Zmień punkt', exact: true }).click();
      await page.getByRole('button', { name: 'Szukaj w innym miejscu', exact: true }).click();
      await page.getByLabel('Szukaj punktu', { exact: true }).fill('Warszawa');
      await page.getByRole('button', { name: 'Szukaj', exact: true }).click();
      await expect(page.locator('.point-list button').first()).toBeVisible();
      const selected = await page.locator('.point-list button strong').first().innerText();
      await page.locator('.point-list button').first().click();
      await expect(page.locator('.selected-point-row')).toContainText(selected);
      expect(selected).not.toBe(first);
      await expect(page.getByLabel('Kod pocztowy', { exact: true })).toHaveCount(1);
    } else {
      await page.getByRole('radio', { name: /Kurier/ }).check();
      await fillStripeAddress(page);
    }
    await expect(page.locator('.embedded-contact')).toContainText('void@sieciowiec.xyz');
    const carriers = page.locator('input[name="carrier"]');
    await expect(carriers.first()).toBeVisible({ timeout: 45_000 });
    await expect(
      page.getByText('Analiza modelu jest wymagana do wyceny dostawy.', { exact: false }),
    ).toHaveCount(0);
    await expect(carriers.first().locator('..')).toHaveCSS('display', 'flex');
    for (let i = 0; i < (await carriers.count()); i++) {
      // Finish scrolling before the native click; the authored page scrolls smoothly.
      await carriers
        .nth(i)
        .evaluate((node) => node.scrollIntoView({ behavior: 'instant', block: 'center' }));
      await carriers.nth(i).check();
      await expect(carriers.nth(i)).toBeChecked();
    }
    const selectedIndex = (await carriers.count()) > 1 ? 1 : 0;
    const selectedCarrier = carriers.nth(selectedIndex);
    await selectedCarrier.check();
    await expect(selectedCarrier).toBeChecked();
    // Radios have no value attribute; their authored label/order maps to server offers.
    const selectedName = await selectedCarrier
      .locator('..')
      .locator('span')
      .evaluate((node) => node.firstChild?.textContent ?? '');
    await page.getByRole('button', { name: 'Podsumowanie →', exact: true }).click();
    await expect(page.locator('.order-stage')).toContainText('Podsumowanie');
    // Only synthetic recipient data; no card, OTP or payment secret is on this screen.
    await page.screenshot({ path: info.outputPath('summary-desktop.png') });
    await page.setViewportSize({ width: 390, height: 844 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
      true,
    );
    await page.screenshot({ path: info.outputPath('summary-mobile.png') });
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.locator('.review-model summary').first().click();
    await expect(page.locator('.review-model').first()).toHaveAttribute('open', '');
    await page.getByRole('button', { name: /Wróć do dostawy/ }).click();
    await expect(page.locator('.embedded-contact')).toContainText('void@sieciowiec.xyz');
    await expect(selectedCarrier).toBeChecked();
    await expect(selectedCarrier.locator('..').locator('span')).toContainText(selectedName);
    await page.getByRole('button', { name: 'Podsumowanie →', exact: true }).click();
    await page.getByRole('button', { name: 'Przejdź do płatności →', exact: true }).click();
    const quote = (await Promise.all(quotes)).at(-1)!;
    expect(quote, 'Payment must use the current observed server quote').toBeDefined();
    const offer = quote.offers[selectedIndex];
    expect(offer.name, 'Checked carrier label/order must match the server offer').toBe(
      selectedName,
    );
    const expected = quote.print_grosze + quote.packaging_grosze + offer.price_grosze;
    expect(Number.isSafeInteger(expected) && expected > 0).toBe(true);
    const payments: {
      input: { quote_id: string; offer_id: string };
      result: Promise<{ total_grosze: number }>;
    }[] = [];
    const intents = new Set<string>();
    page.on('response', async (response) => {
      if (response.url().endsWith('/api/checkout/intent') && response.ok()) {
        payments.push({ input: response.request().postDataJSON(), result: response.json() });
        const result = await response.json();
        intents.add(result.payment_intent_id);
      }
    });
    if (method === 'locker') {
      await fillStripeCard(page, '4000000000000002');
      // Lose a real server response after it has created the test PaymentIntent.
      await page.route(
        '**/api/checkout/intent',
        async (route) => {
          const real = await route.fetch();
          expect(real.status()).toBe(200);
          payments.push({ input: route.request().postDataJSON(), result: real.json() });
          intents.add((await real.json()).payment_intent_id);
          await route.abort('failed');
        },
        { times: 1 },
      );
      await page.getByRole('button', { name: /^Zamawiam i płacę/ }).click();
      await expect(page.getByRole('alert').first()).toBeVisible();
      await page.getByRole('button', { name: /^Zamawiam i płacę/ }).click();
      await expect(page.getByRole('alert').first()).toContainText(/odrzucon|declin/i, {
        timeout: 30_000,
      });
      await expect(page.getByRole('alert').first()).toBeVisible({ timeout: 30_000 });
      expect(intents.size).toBe(1);
      const id = [...intents][0];
      expect((await (await page.request.get(`/api/checkout/intent/${id}`)).json()).status).not.toBe(
        'paid',
      );
    }
    await fillStripeCard(page);
    await page.getByRole('button', { name: /^Zamawiam i płacę/ }).click();
    await expect
      .poll(
        async () => {
          const id = new URL(page.url()).searchParams.get('payment_intent');
          if (!id) return 'missing';
          return (await (await page.request.get(`/api/checkout/intent/${id}`)).json()).status;
        },
        { timeout: 35_000 },
      )
      .toBe('paid');
    await expect(
      page.getByRole('heading', { name: 'Płatność potwierdzona.', exact: true }),
    ).toBeVisible();
    expect(intents.size).toBe(1);
    expect(payments.length).toBeGreaterThan(0);
    for (const payment of payments) {
      expect(payment.input).toMatchObject({ quote_id: quote.id, offer_id: offer.id });
      expect((await payment.result).total_grosze).toBe(expected);
    }
    const paidId = [...intents][0];
    const status = await (await page.request.get(`/api/checkout/intent/${paidId}`)).json();
    expect(status).toMatchObject({ status: 'paid', quote_id: quote.id, total_grosze: expected });
    const receipt = await page.request.get(`/api/account/orders/${status.order_id}`);
    expect(receipt.status()).toBe(200);
    const order = await receipt.json();
    expect(order.amount_grosze).toBe(expected);
    expect(order.fulfillment.offer).toMatchObject({
      id: offer.id,
      name: offer.name,
      price_grosze: offer.price_grosze,
      total_grosze: expected,
    });
    await page.reload();
    await expect
      .poll(
        async () =>
          (await (await page.request.get(`/api/checkout/intent/${paidId}`)).json()).status,
      )
      .toBe('paid');
    await expect(page.getByRole('button', { name: /^Zamawiam i płacę/ })).toHaveCount(0);
  });
}
