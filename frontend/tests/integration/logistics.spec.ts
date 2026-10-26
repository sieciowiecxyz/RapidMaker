import { expect, test } from './fixtures';
import { accounts, login, fillStripeCard } from './helpers';
import { configureCheckout, fillStripeAddress } from './checkout-helpers';
import { readFile } from 'node:fs/promises';

test.use({ screenshot: 'off', trace: 'off' });

for (const finish of ['cancel', 'handover'])
  test(`real registered courier order books a sandbox label and ${finish}`, async ({
    page,
    browser,
  }, info) => {
    test.skip(process.env.RAPIDMAKER_SANDBOX !== '1', 'Requires actual test providers');
    test.setTimeout(240_000);
    const fixture = accounts();
    await login(
      page,
      fixture.find((a) => a.email === 'void@sieciowiec.xyz')!,
    );
    await configureCheckout(page);
    await fillStripeAddress(page);
    const carrier = page.getByRole('radio', { name: /^DHL/ });
    await expect(carrier).toBeVisible({ timeout: 45_000 });
    await carrier.check();
    await page.getByRole('button', { name: 'Podsumowanie →', exact: true }).click();
    await page.getByRole('button', { name: 'Przejdź do płatności →', exact: true }).click();
    await fillStripeCard(page);
    await page.getByRole('button', { name: /^Zamawiam i płacę/ }).click();
    let orderId = '';
    await expect
      .poll(
        async () => {
          const id = new URL(page.url()).searchParams.get('payment_intent');
          if (!id) return 'missing';
          const result = await (await page.request.get(`/api/checkout/intent/${id}`)).json();
          orderId = result.order_id;
          return result.status;
        },
        { timeout: 35_000 },
      )
      .toBe('paid');
    expect(orderId).toMatch(/^[0-9a-f-]{36}$/);
    const detailsLink = page.getByRole('link', { name: 'Szczegóły zamówienia →', exact: true });
    await expect(detailsLink).toHaveAttribute('href', '/konto/zamowienia/' + orderId);
    await detailsLink.click();
    await expect(page.getByLabel(/Podgląd modelu cube_20mm.stl/)).toBeVisible();

    const operatorContext = await browser.newContext();
    try {
      const operator = await operatorContext.newPage();
      await login(
        operator,
        fixture.find((a) => a.role === 'admin')!,
      );
      await operator.goto('/admin.html');
      await operator.getByRole('button', { name: `Otwórz ${orderId}`, exact: true }).click();
      const detail = operator.getByRole('complementary', { name: `Szczegóły ${orderId}` });
      await detail.getByLabel('Etap realizacji').selectOption('production:printing');
      await expect
        .poll(
          async () =>
            (await (await page.request.get(`/api/account/orders/${orderId}`)).json()).status,
        )
        .toBe('in_production');
      await detail
        .getByRole('spinbutton', { name: /Gotowe \/ / })
        .first()
        .fill('1');
      await detail
        .getByRole('spinbutton', { name: /Gotowe \/ / })
        .first()
        .press('Tab');
      await expect
        .poll(
          async () =>
            (await (await operator.request.get(`/api/admin/orders/${orderId}/production`)).json())
              .progress[0]?.done,
        )
        .toBe(1);
      await detail.getByLabel('Etap realizacji').selectOption('production:qc');
      await detail
        .getByRole('checkbox', {
          name: 'Kontrola jakości zakończona — wszystkie sztuki sprawdzone',
          exact: true,
        })
        .check();
      await expect
        .poll(
          async () =>
            (await (await operator.request.get(`/api/admin/orders/${orderId}/production`)).json())
              .qc_passed,
        )
        .toBe(true);
      await detail.getByLabel('Etap realizacji').selectOption('production:packing');
      await detail.getByRole('button', { name: /^Dalej:/ }).click();
      await expect
        .poll(
          async () =>
            (await (await page.request.get(`/api/account/orders/${orderId}`)).json()).status,
        )
        .toBe('ready');
      await detail.getByRole('button', { name: 'Dostawa', exact: true }).click();
      const shipping = detail.getByRole('region', { name: 'Dostawa zamówienia' });
      await expect(shipping.getByLabel('Długość', { exact: true }).first()).toBeVisible();
      await shipping.getByRole('button', { name: '+ Dodaj paczkę', exact: true }).click();
      await shipping.getByRole('button', { name: 'Zapisz paczki', exact: true }).click();
      expect(
        await shipping
          .getByLabel('Długość', { exact: true })
          .nth(1)
          .evaluate((input: HTMLInputElement) => input.validity.rangeUnderflow),
      ).toBe(true);
      await shipping.getByRole('button', { name: 'Usuń paczkę', exact: true }).last().click();
      await shipping.getByRole('button', { name: '+ Dodaj paczkę', exact: true }).click();
      for (const [label, value] of [
        ['Długość', '180'],
        ['Szerokość', '130'],
        ['Wysokość', '90'],
        ['Masa', '200'],
      ])
        await shipping.getByLabel(label, { exact: true }).nth(1).fill(value);
      await shipping.getByRole('checkbox', { name: /Potwierdzam zakres wysyłki/ }).check();
      await shipping.getByRole('button', { name: 'Zapisz paczki', exact: true }).click();
      await expect(
        shipping.getByRole('button', { name: 'Sprawdź koszt przewozu', exact: true }),
      ).toBeEnabled();
      await shipping.getByRole('button', { name: 'Sprawdź koszt przewozu', exact: true }).click();
      const book = shipping.getByRole('button', { name: /^Zamów przewóz za/ });
      await expect(book).toBeDisabled();
      await shipping.getByRole('checkbox', { name: /Akceptuję koszt/ }).check();
      await book.click();
      const endpoint = `/api/admin/orders/${orderId}/shipment`;
      await expect
        .poll(async () => (await (await operator.request.get(endpoint)).json()).shipment?.state, {
          timeout: 70_000,
          intervals: [1000, 2000],
        })
        .toMatch(/^(booked|manual_review)$/);
      expect((await (await operator.request.get(endpoint)).json()).shipment.state).toBe('booked');
      await shipping.getByRole('button', { name: 'Odśwież', exact: true }).click();
      const download = operator.waitForEvent('download');
      await shipping.getByRole('button', { name: 'Etykieta PDF', exact: true }).first().click();
      const label = await download;
      expect((await readFile((await label.path())!)).subarray(0, 4).toString()).toBe('%PDF');
      await shipping.getByRole('checkbox', { name: /Przesyłka 1/ }).check();
      if (finish === 'handover') {
        // Booking precedes the asynchronous retrieval of tracking numbers.
        await expect
          .poll(
            async () => {
              const snapshot = await (await operator.request.get(endpoint)).json();
              return snapshot.shipment.packages.every(
                (p: { tracking: { numbers?: string[] } }) => (p.tracking.numbers?.length ?? 0) > 0,
              );
            },
            { timeout: 45_000, intervals: [1000, 2000] },
          )
          .toBe(true);
        await shipping.getByRole('button', { name: 'Odśwież', exact: true }).click();
        operator.once('dialog', (dialog) => dialog.accept());
        await shipping
          .getByRole('button', { name: 'Potwierdź przekazanie kurierowi', exact: true })
          .click();
        await expect
          .poll(
            async () =>
              (await (await page.request.get(`/api/account/orders/${orderId}`)).json()).status,
          )
          .toBe('shipped');
        const final = await (await operator.request.get(endpoint)).json();
        expect(final.shipment.packages).toHaveLength(1);
        expect(final.shipment.packages[0].state).toBe('dispatched');
        return;
      }
      await shipping.getByText('Podjazd kuriera', { exact: true }).click();
      const readyDate = new Date(Date.now() + 86400_000).toLocaleDateString('sv-SE', {
        timeZone: 'Europe/Warsaw',
      });
      await shipping.getByLabel('Gotowe do odbioru od', { exact: true }).fill(readyDate);
      await shipping
        .getByRole('checkbox', { name: 'Ponowny podjazd po nieudanym odbiorze', exact: true })
        .check();
      await shipping
        .getByRole('checkbox', { name: 'Ponowny podjazd po nieudanym odbiorze', exact: true })
        .uncheck();
      await shipping.getByRole('button', { name: 'Pokaż dostępne terminy', exact: true }).click();
      await expect(shipping.getByRole('combobox', { name: 'Termin', exact: true })).toBeVisible();
      await shipping.getByRole('combobox', { name: 'Termin', exact: true }).selectOption('0');
      await shipping.getByRole('button', { name: 'Zamów podjazd', exact: true }).click();
      let pickup: { state: string; result: unknown } | undefined;
      await expect
        .poll(
          async () => {
            const history = await (await operator.request.get(endpoint + '/actions')).json();
            pickup = history.find((a: { kind: string }) => a.kind === 'pickup');
            return pickup?.state;
          },
          { timeout: 70_000, intervals: [1000, 2000] },
        )
        .toMatch(/^(succeeded|failed|manual_review|partial)$/);
      if (pickup!.state !== 'succeeded') {
        // Known sandbox carrier limitation: assert the rejection, never claim a confirmed pickup.
        expect(JSON.stringify(pickup!.result)).toContain('carrierConnectionError');
        info.annotations.push({
          type: 'provider-blocked',
          description:
            'Carrier sandbox pickup rejected: carrierConnectionError; pickup not confirmed',
        });
      }
      await shipping.getByRole('button', { name: 'Odśwież', exact: true }).click();
      await shipping.getByRole('checkbox', { name: /Przesyłka 1/ }).check();
      await shipping.getByText('Anulowanie zaznaczonych przesyłek', { exact: true }).click();
      await shipping.getByRole('checkbox', { name: /Potwierdzam anulowanie/ }).check();
      await shipping.getByRole('button', { name: 'Zleć anulowanie', exact: true }).click();
      await expect
        .poll(
          async () => {
            const history = await (await operator.request.get(endpoint + '/actions')).json();
            return history.find((a: { kind: string }) => a.kind === 'cancel')?.state;
          },
          { timeout: 70_000, intervals: [1000, 2000] },
        )
        .toBe('succeeded');
      await shipping.getByRole('button', { name: 'Odśwież', exact: true }).click();
      // Two physical parcels are one courier consignment; cancellation leaves the order unfulfilled.
      await expect
        .poll(
          async () =>
            (await (await page.request.get(`/api/account/orders/${orderId}`)).json()).status,
        )
        .toBe('ready');
      const final = await (await operator.request.get(endpoint)).json();
      expect(final.shipment.packages).toHaveLength(1);
      expect(final.shipment.packages[0].state).toBe('canceled');
      await expect(
        shipping.getByRole('button', { name: 'Potwierdź przekazanie kurierowi', exact: true }),
      ).toBeDisabled();
    } finally {
      await operatorContext.close();
    }
  });
