import { expect, test, type Page } from '@playwright/test';
const id = '00000000-0000-4000-8000-000000000002';
async function admin(page: Page, booked = false) {
  const writes: { path: string; body: any; csrf?: string }[] = [];
  let shipment: any = null,
    loseBooking = true,
    loseRefund = true;
  const refunds: any[] = [],
    actions: any[] = [];
  const parcels = [{ outer_mm: [300, 200, 100], weight_g: 500 }];
  const order = {
    id,
    status: 'ready',
    payment_state: 'paid',
    amount_grosze: 14555,
    created_at: 1792840000,
    paid_at: 1792840200,
    contact: {
      name: 'Klient API',
      email: 'client@example.invalid',
      street: 'Testowa 2',
      postalCode: '05-500',
      city: 'Piaseczno',
    },
    lines: [{ model_id: 'model-1', quantity: 3, unit_grosze: 18, total_grosze: 52 }],
    fulfillment: {
      delivery: { method: 'courier' },
      offer: { name: 'DPD', price_grosze: 1555 },
      packing: { parcels },
    },
  };
  if (booked)
    shipment = {
      id: 'shipment-1',
      version: 1,
      state: 'booked',
      input: { parcels, refund_reviewed: false },
      quote: null,
      packages: [{ id: 'package-1', reference: 'RM1', provider_id: '12345', state: 'booked' }],
      last_error: null,
    };
  await page.route('**/api/**', async (route) => {
    const req = route.request(),
      path = new URL(req.url()).pathname,
      method = req.method();
    if (method !== 'GET')
      writes.push({ path, body: req.postDataJSON(), csrf: req.headers()['x-csrf-token'] });
    let data: unknown;
    if (path === '/api/auth/me')
      data = {
        user: { id: 'admin-1', email: 'operator@example.invalid', role: 'admin' },
        stage: 'authenticated',
        second_factor: 'email',
        admin_verified: true,
        csrf_token: 'csrf-operations',
      };
    else if (path === '/api/admin/orders') data = { orders: [order] };
    else if (path.endsWith('/files')) data = { files: [] };
    else if (path.endsWith('/refunds')) {
      if (method === 'POST') {
        if (loseRefund) {
          loseRefund = false;
          return route.abort('failed');
        }
        const input = req.postDataJSON();
        const existing = refunds.find((r) => r.request_id === input.request_id);
        data = existing ?? {
          id: 'refund-1',
          order_id: id,
          ...input,
          state: 'queued',
          created_at: 1792840300,
        };
        if (!existing) refunds.push(data);
      } else data = refunds;
    } else if (path.endsWith('/shipment')) {
      if (method === 'PUT')
        shipment = {
          id: 'shipment-1',
          version: 1,
          state: 'draft',
          input: { ...req.postDataJSON() },
          quote: null,
          packages: [],
          last_error: null,
        };
      data = method === 'GET' ? { order, shipment } : shipment;
    } else if (path.endsWith('/quote')) {
      shipment.quote = {
        id: 'quote-1',
        expires_at: Math.floor(Date.now() / 1000) + 120,
        price_grosze: 1700,
        collected_shipping_grosze: 1555,
        difference_grosze: 145,
      };
      data = shipment;
    } else if (path.endsWith('/book')) {
      if (loseBooking) {
        loseBooking = false;
        return route.abort('failed');
      }
      shipment.state = 'booked';
      shipment.packages = [
        { id: 'package-1', reference: 'RM1', provider_id: '12345', state: 'booked' },
      ];
      data = shipment;
    } else if (path.endsWith('/actions')) data = actions;
    else if (path.includes('/labels/'))
      return route.fulfill({
        contentType: 'application/pdf',
        body: Buffer.from('%PDF-1.4\n%%EOF'),
      });
    else if (path.endsWith('/pickup/proposals'))
      data = {
        packages: [
          {
            package_id: 12345,
            proposals: [
              { available: true, date: '2026-10-27', min_time: '09:00', max_time: '17:00' },
            ],
          },
        ],
      };
    else if (path.endsWith('/pickup') || path.endsWith('/cancel')) {
      data = {
        id: 'action-' + (actions.length + 1),
        request_id: req.postDataJSON().request_id,
        kind: path.endsWith('/pickup') ? 'pickup' : 'cancel',
        state: 'queued',
        result: {},
      };
      actions.push(data);
    } else throw new Error('Unexpected API ' + method + ' ' + path);
    return route.fulfill({
      status:
        method === 'POST' && (path.endsWith('/book') || path.endsWith('/refunds')) ? 202 : 200,
      json: data,
    });
  });
  await page.goto('/admin.html');
  await page.getByRole('button', { name: `Otwórz ${id}`, exact: true }).click();
  return writes;
}

test('shipment approval, ambiguous retry, label and pickup use real API contracts', async ({
  page,
}) => {
  const writes = await admin(page);
  await page.getByRole('button', { name: 'Dostawa', exact: true }).click();
  await expect(page.getByLabel('Długość', { exact: true })).toHaveValue('300');
  expect(writes).toHaveLength(0);
  await page.getByRole('button', { name: 'Zapisz paczki', exact: true }).click();
  await page.getByRole('button', { name: 'Sprawdź koszt przewozu', exact: true }).click();
  const book = page.getByRole('button', { name: 'Zamów przewóz za 17,00 zł', exact: true });
  await expect(book).toBeDisabled();
  await page.getByLabel('Akceptuję koszt', { exact: false }).check();
  await book.click();
  await expect(
    page.getByRole('button', { name: 'Sprawdź / ponów tę samą operację', exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Sprawdź / ponów tę samą operację', exact: true }).click();
  await expect(page.getByText('Przewóz zamówiony', { exact: true })).toBeVisible();
  const bookings = writes.filter((w) => w.path.endsWith('/book'));
  expect(bookings).toHaveLength(2);
  expect(bookings[0].body).toEqual(bookings[1].body);
  const downloaded = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Etykieta PDF', exact: true }).click();
  expect((await downloaded).suggestedFilename()).toBe('rapidmaker-package-1.pdf');
  await page.getByLabel('Przesyłka 1', { exact: false }).check();
  await page.getByText('Podjazd kuriera', { exact: true }).click();
  await page.getByLabel('Gotowe do odbioru od').fill('2026-10-27');
  await page.getByRole('button', { name: 'Pokaż dostępne terminy', exact: true }).click();
  await page.getByRole('combobox', { name: 'Termin', exact: true }).selectOption('0');
  await page.getByRole('button', { name: 'Zamów podjazd', exact: true }).click();
  await expect(page.getByText('Operacje przewoźnika', { exact: true })).toBeVisible();
  expect(writes.find((w) => w.path.endsWith('/pickup'))?.body.pickup_date).toEqual({
    date: '2026-10-27',
    min_time: '09:00',
    max_time: '17:00',
  });
  expect(writes.every((w) => w.csrf === 'csrf-operations')).toBe(true);
});

test('partial refund survives page reload without a second operation', async ({ page }) => {
  page.on('dialog', (d) => d.accept());
  const writes = await admin(page);
  await page.getByRole('button', { name: 'Rozliczenie', exact: true }).click();
  const amount = page.getByLabel('Kwota zwrotu (zł)', { exact: true });
  await amount.fill('10,25');
  await page.getByRole('button', { name: 'Zleć zwrot', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Ponów to zlecenie', exact: true })).toBeVisible();
  await page.reload();
  await page.getByRole('button', { name: `Otwórz ${id}`, exact: true }).click();
  await page.getByRole('button', { name: 'Rozliczenie', exact: true }).click();
  await expect(amount).toHaveCount(0);
  await page.getByRole('button', { name: 'Ponów to zlecenie', exact: true }).click();
  await expect(page.getByText('Zlecony', { exact: true })).toBeVisible();
  const attempts = writes.filter((w) => w.path.endsWith('/refunds'));
  expect(attempts).toHaveLength(2);
  expect(attempts[0].body).toEqual(attempts[1].body);
  expect(attempts[0].body.amount_grosze).toBe(1025);
  await page.getByRole('button', { name: 'Cała pozostała kwota', exact: true }).click();
  await expect(amount).toHaveValue('135,30');
});

test('first passkey uses native WebAuthn, clears grant and protects last key', async ({
  page,
  context,
}) => {
  const cdp = await context.newCDPSession(page);
  await cdp.send('WebAuthn.enable');
  await cdp.send('WebAuthn.addVirtualAuthenticator', {
    options: {
      protocol: 'ctap2',
      transport: 'internal',
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });
  let registered = false;
  const writes: any[] = [];
  await page.route('**/api/**', async (route) => {
    const req = route.request(),
      path = new URL(req.url()).pathname;
    let data: unknown;
    if (req.method() !== 'GET')
      writes.push({ path, body: req.postDataJSON(), csrf: req.headers()['x-csrf-token'] });
    if (path === '/api/auth/me')
      data = {
        user: {
          id: 'client-1',
          email: 'client@example.invalid',
          role: registered ? 'admin' : 'client',
        },
        stage: 'authenticated',
        second_factor: 'email',
        admin_verified: registered,
        csrf_token: 'csrf-key',
      };
    else if (path === '/api/account/security')
      data = {
        has_password: false,
        totp_enabled: false,
        passkeys: registered ? [{ id: 'key-1', name: 'Mój klucz', created_at: 1792840000 }] : [],
      };
    else if (path === '/api/account/notifications')
      data = { email_order_updates: true, email_chat_messages: true };
    else if (path.endsWith('/register/start'))
      data = {
        challenge_id: 'challenge-1',
        options: {
          publicKey: {
            challenge: 'AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQ',
            rp: { name: 'Rapidmaker', id: 'localhost' },
            user: { id: 'Y2xpZW50LTE', name: 'client@example.invalid', displayName: 'Klient' },
            pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
            authenticatorSelection: { userVerification: 'required' },
            timeout: 60000,
            attestation: 'none',
          },
        },
      };
    else if (path.endsWith('/register/finish')) {
      registered = true;
      data = {
        user: { id: 'client-1', email: 'client@example.invalid', role: 'admin' },
        stage: 'authenticated',
        second_factor: 'email',
        admin_verified: true,
        csrf_token: 'csrf-key',
      };
    } else if (path === '/api/auth/passkeys')
      data = { passkeys: [{ id: 'key-1', name: 'Mój klucz', created_at: 1792840000 }] };
    else throw new Error('Unexpected API ' + path);
    return route.fulfill({ json: data });
  });
  await page.goto('http://localhost:18431/konto/ustawienia?bootstrap=1');
  await page.getByLabel('Nazwa klucza', { exact: true }).fill('Mój klucz');
  await page.getByLabel('Jednorazowe zezwolenie', { exact: true }).fill('test-local-grant');
  await page.getByRole('button', { name: 'Zarejestruj klucz operatora', exact: true }).click();
  await expect(page.getByText('Klucz dostępu został zapisany.', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Usuń klucz', exact: true })).toBeDisabled();
  expect(writes.find((w) => w.path.endsWith('/register/start')).body.bootstrap).toBe(
    'test-local-grant',
  );
  expect(
    writes.find((w) => w.path.endsWith('/register/finish')).body.credential.response
      .attestationObject,
  ).toBeTruthy();
  expect(writes.every((w) => w.csrf === 'csrf-key')).toBe(true);
  expect(
    await page.evaluate(() => JSON.stringify({ ...sessionStorage, ...localStorage })),
  ).not.toContain('test-local-grant');
});

test('cancellation requires explicit approval and remains pending; compact mobile layout', async ({
  page,
}) => {
  const writes = await admin(page, true);
  await page.getByRole('button', { name: 'Dostawa', exact: true }).click();
  await page.getByLabel('Przesyłka 1', { exact: false }).check();
  await page.getByText('Anulowanie zaznaczonych przesyłek', { exact: true }).click();
  const cancel = page.getByRole('button', { name: 'Zleć anulowanie', exact: true });
  await expect(cancel).toBeDisabled();
  await page.screenshot({ path: 'tests/artifacts/shipping-desktop.png' });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(cancel).toBeVisible();
  await page.screenshot({ path: 'tests/artifacts/shipping-mobile.png' });
  await page.getByLabel('Potwierdzam anulowanie 1 przesyłek.', { exact: false }).check();
  await cancel.click();
  await expect(page.getByText('Operacje przewoźnika', { exact: true })).toBeVisible();
  const calls = writes.filter((w) => w.path.endsWith('/cancel'));
  expect(calls).toHaveLength(1);
  expect(calls[0].body.package_ids).toEqual(['package-1']);
  expect(calls[0].body.request_id).toMatch(/^[a-f0-9-]{36}$/);
  expect(calls[0].csrf).toBe('csrf-operations');
  await expect(page.getByText('Anulowano', { exact: true })).toHaveCount(0);
});
