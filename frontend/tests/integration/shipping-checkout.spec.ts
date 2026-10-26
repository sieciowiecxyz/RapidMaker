import { expect, test } from './fixtures';
import { configureCheckout, fillStripeAddress } from './checkout-helpers';
import { fillStripeCard } from './helpers';

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
    await configureCheckout(page);
    // Pickup exposes the native recipient inputs without duplicating Stripe's address form.
    await page.getByRole('radio', { name: /Odbiór osobisty/ }).check();
    await page.getByLabel('Imię i nazwisko', { exact: true }).fill('Test Rapidmaker');
    await page.getByLabel('Email', { exact: true }).fill('void@sieciowiec.xyz');
    await page.getByLabel('Telefon', { exact: true }).fill('500600700');
    await page.getByRole('checkbox', { name: 'Chcę fakturę na firmę', exact: true }).check();
    await page.getByLabel('Nazwa firmy', { exact: true }).fill('Firma testowa');
    const nip = page.getByLabel('NIP', { exact: true });
    await nip.fill('123');
    expect(await nip.evaluate((el: HTMLInputElement) => el.validity.valid)).toBe(false);
    await nip.fill('1234567883');
    await page.getByRole('checkbox', { name: 'Chcę fakturę na firmę', exact: true }).uncheck();
    await expect(nip).toHaveCount(0);
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
    await expect(page.getByLabel('Email', { exact: true })).toHaveValue('void@sieciowiec.xyz');
    const carriers = page.locator('input[name="carrier"]');
    await expect(carriers.first()).toBeVisible({ timeout: 45_000 });
    for (let i = 0; i < (await carriers.count()); i++) {
      await carriers.nth(i).check();
      await expect(carriers.nth(i)).toBeChecked();
    }
    await carriers.first().check();
    await page.getByRole('button', { name: 'Podsumowanie →', exact: true }).click();
    await expect(
      page.getByRole('heading', { name: 'Sprawdź zamówienie.', exact: true }),
    ).toBeVisible();
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
    await expect(page.getByLabel('Email', { exact: true })).toHaveValue('void@sieciowiec.xyz');
    await page.getByRole('button', { name: 'Podsumowanie →', exact: true }).click();
    await page.getByRole('button', { name: 'Przejdź do płatności →', exact: true }).click();
    const intents = new Set<string>();
    page.on('response', async (response) => {
      if (response.url().endsWith('/api/checkout/intent') && response.ok()) {
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
    const paidId = [...intents][0];
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
