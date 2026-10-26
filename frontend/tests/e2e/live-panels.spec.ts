import { expect, test, type Page } from '@playwright/test';
const id = '00000000-0000-4000-8000-000000000001';
async function fixture(page: Page, role = 'client', verified = true) {
  let stage = verified ? 'authenticated' : 'second_factor';
  let authenticated = true,
    state = 'paid',
    lostRefund = false,
    orderFetches = 0;
  const writes: { path: string; csrf?: string; body: any }[] = [];
  const attempts: unknown[] = [],
    refunds: unknown[] = [];
  const messages = [
    { id: 'm1', sequence: 1, operator: true, body: 'Model sprawdzony.', created_at: 1792840200 },
  ];
  let prefs = { email_order_updates: true, email_chat_messages: false };
  let production = {
    revision: 0,
    stage: 'preparation',
    due_date: null,
    private_notes: '',
    progress: [],
    qc_passed: false,
  };
  const raw = () => ({
    production,
    id,
    status: state,
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
      packing: { parcels: [{}] },
    },
  });
  await page.route('**/api/**', async (route) => {
    const req = route.request(),
      url = new URL(req.url()),
      path = url.pathname,
      method = req.method();
    if (method !== 'GET')
      writes.push({ path, csrf: req.headers()['x-csrf-token'], body: req.postDataJSON() });
    let data: unknown;
    if (path === '/api/auth/me') {
      if (!authenticated) return route.fulfill({ status: 401, json: { message: 'Zaloguj się.' } });
      data = {
        user: { id: role + '-1', email: 'client@example.invalid', role },
        admin_verified: verified,
        stage,
        second_factor: 'email',
        csrf_token: 'csrf-contract',
      };
    } else if (path === '/api/auth/password/login') {
      authenticated = true;
      stage = 'second_factor';
      data = {
        user: { id: 'client-1', email: 'client@example.invalid', role },
        stage,
        second_factor: 'email',
        admin_verified: false,
        csrf_token: 'csrf-contract',
      };
    } else if (path === '/api/auth/factor/verify') {
      stage = 'authenticated';
      data = {
        user: { id: 'client-1', email: 'client@example.invalid', role },
        stage,
        second_factor: 'email',
        admin_verified: role === 'admin',
        csrf_token: 'csrf-contract',
      };
    } else if (path === '/api/auth/start') data = { challenge_id: 'challenge-1' };
    else if (path === '/api/auth/verify') {
      authenticated = true;
      data = {
        user: { id: 'client-1', email: 'client@example.invalid', role: 'client' },
        admin_verified: false,
        csrf_token: 'csrf-contract',
      };
    } else if (path === '/api/auth/claim-guest') data = { resources_claimed: 1 };
    else if (path === '/api/account/security')
      data = { has_password: true, totp_enabled: false, passkeys: [] };
    else if (path === '/api/auth/passkeys') data = { passkeys: [] };
    else if (path === '/api/account/notifications') {
      if (method === 'PUT') prefs = req.postDataJSON();
      data = prefs;
    } else if (path === '/api/admin/orders' || path === '/api/account/orders') {
      orderFetches++;
      data = { orders: [raw()] };
    } else if (path === `/api/account/orders/${id}`) data = raw();
    else if (path.endsWith('/files'))
      data = {
        files: [
          {
            model_id: 'model-1',
            filename: 'uchwyt.stl',
            material_id: 'PLA',
            color_code: '#ffffff',
            quantity: 3,
            available: true,
            source_url: `${path}/model-1/source`,
            dimensions_mm: [120, 80, 45],
            options: { quality: 'fine', wall_loops: 3, infill_percent: 20 },
          },
        ],
      };
    else if (path.endsWith('/shipment'))
      data = role === 'admin' ? { shipment: null } : { state: 'not_prepared', packages: [] };
    else if (path.endsWith('/messages/read')) data = { ok: true };
    else if (path.endsWith('/messages')) {
      if (method === 'POST') {
        messages.push({
          id: 'm' + (messages.length + 1),
          sequence: messages.length + 1,
          operator: role === 'admin',
          body: req.postDataJSON().body,
          created_at: 1792840300,
        });
        data = { ok: true };
      } else
        data = {
          messages: messages.filter((m) => m.sequence > Number(url.searchParams.get('after') || 0)),
          next_after: null,
          read_sequence: 0,
          unread: 1,
        };
    } else if (path.endsWith('/production')) {
      if (method === 'PUT') {
        production = { ...req.postDataJSON(), revision: production.revision + 1 };
        if (production.stage === 'printing') state = 'in_production';
      }
      data = production;
    } else if (path.endsWith('/status')) {
      state = req.postDataJSON().status;
      data = { ok: true };
    } else if (path.endsWith('/refunds')) {
      if (method === 'POST') {
        attempts.push(req.postDataJSON());
        if (!refunds.length) refunds.push({ ...req.postDataJSON(), state: 'queued' });
        if (lostRefund) {
          lostRefund = false;
          return route.abort('failed');
        }
        data = refunds[0];
      } else data = refunds;
    } else if (path === '/api/checkout/config') data = { publishable_key: null };
    else if (path === '/api/catalog/materials') data = { materials: [] };
    else throw new Error('Unexpected API ' + method + ' ' + path);
    return route.fulfill({
      status: method === 'POST' && path.endsWith('/refunds') ? 202 : 200,
      json: data,
    });
  });
  return {
    writes,
    attempts,
    orderFetches: () => orderFetches,
    signOut: () => {
      authenticated = false;
    },
    loseRefund: () => {
      lostRefund = true;
    },
  };
}

test('password plus email code connects customer orders, files, chat and notification preferences', async ({
  page,
}) => {
  const api = await fixture(page);
  api.signOut();
  await page.goto('/logowanie');
  await page.getByLabel('Adres email').fill('client@example.invalid');
  await page.getByLabel('Hasło', { exact: true }).fill('correct horse battery');
  await page.getByRole('button', { name: 'Zaloguj hasłem', exact: true }).click();
  expect(api.orderFetches()).toBe(0);
  await page.getByLabel('Kod z emaila').fill('12345678');
  await expect(page.getByLabel('Kod z emaila')).toHaveAttribute('pattern', '[0-9]{8}');
  await page.getByRole('button', { name: 'Potwierdź i zaloguj', exact: true }).click();
  await expect(page).toHaveURL(/\/konto\/zamowienia$/);
  await expect(page.locator('.price')).toContainText('145,55');
  await page.locator(`a[href="/konto/zamowienia/${id}"]`).click();
  await expect(page.getByRole('heading', { name: 'Rozmowa o zamówieniu' })).toBeVisible();
  await expect(page.getByRole('link', { name: /Pobierz/ })).toHaveAttribute(
    'href',
    `/api/account/orders/${id}/files/model-1/source`,
  );
  await page.getByLabel('Wiadomość', { exact: true }).fill('Proszę o zielony wydruk.');
  await page.getByRole('button', { name: 'Wyślij wiadomość', exact: true }).click();
  await expect(page.locator('.messages')).toContainText('Proszę o zielony wydruk.');
  await expect(page.getByRole('button', { name: /Zleć zwrot/ })).toHaveCount(0);
  await page.getByRole('link', { name: 'Ustawienia konta', exact: true }).click();
  await page.getByLabel('Nowe wiadomości od obsługi').check();
  await page.getByRole('button', { name: 'Zapisz ustawienia', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Zapisano preferencje');
  expect(api.writes.find((w) => w.path.endsWith('/claim-guest'))).toBeDefined();
  expect(
    api.writes
      .filter(
        (w) =>
          !['/api/auth/start', '/api/auth/verify', '/api/auth/password/login'].includes(w.path),
      )
      .every((w) => w.csrf === 'csrf-contract'),
  ).toBe(true);
});

test('admin does not fetch orders before MFA despite a browser role', async ({ page }) => {
  const api = await fixture(page, 'admin', false);
  await page.addInitScript(() =>
    localStorage.setItem(
      'rapidmaker.session',
      JSON.stringify({ role: 'admin', token: 'old-token' }),
    ),
  );
  await page.goto('/admin.html');
  await expect(page.getByLabel('Kod z emaila', { exact: true })).toBeVisible();
  expect(api.orderFetches()).toBe(0);
  await expect(page.getByText('Klient API', { exact: true })).toHaveCount(0);
});

test('admin status and chat use CSRF, exact totals and clear expired data', async ({ page }) => {
  const api = await fixture(page, 'admin');
  await page.goto('/admin.html');
  await page.getByRole('button', { name: `Otwórz ${id}`, exact: true }).click();
  await expect(page.getByText('0,52', { exact: false })).toBeVisible();
  await expect(page.locator('input[type=date]')).toBeEnabled();
  await page.getByRole('button', { name: 'Dalej: Drukowanie', exact: true }).click();
  await expect(page.locator('.detail-summary select')).toHaveValue('production:printing');
  await page.getByRole('button', { name: 'Wiadomości', exact: true }).last().click();
  await page.getByLabel('Twoja odpowiedź').fill('Wydruk rozpoczęty.');
  await page.getByRole('button', { name: 'Wyślij wiadomość', exact: true }).click();
  await expect(page.getByText('Wydruk rozpoczęty.', { exact: true })).toBeVisible();
  expect(api.writes.every((w) => w.csrf === 'csrf-contract')).toBe(true);
  await page.keyboard.press('Escape');
  api.signOut();
  await page.getByRole('button', { name: 'Odśwież', exact: true }).click();
  await expect(page).toHaveURL(/\/logowanie\?/);
  await expect(page.getByText('Klient API', { exact: true })).toHaveCount(0);
});

test('refund retry after lost response reuses request id and remains pending', async ({ page }) => {
  const api = await fixture(page, 'admin');
  api.loseRefund();
  page.on('dialog', (d) => d.accept());
  await page.goto(`/admin/zamowienia/${id}`);
  const refund = page.getByRole('button', { name: /Zleć zwrot 145,55/ });
  await refund.click();
  await expect(page.getByRole('alert')).toBeVisible();
  await refund.click();
  await expect(
    page.getByText('Zwrot został zlecony i czeka na potwierdzenie Stripe.', { exact: true }),
  ).toBeVisible();
  expect(api.attempts).toHaveLength(2);
  expect(api.attempts[0]).toEqual(api.attempts[1]);
  expect(
    api.writes.filter((w) => w.path.endsWith('/refunds')).every((w) => w.csrf === 'csrf-contract'),
  ).toBe(true);
});

for (const scenario of [
  {
    name: 'admin email link',
    start: `/admin/zamowienia/${id}#rozmowa`,
    target: `/admin/zamowienia/${id}#rozmowa`,
    role: 'admin',
  },
  {
    name: 'client rejection of admin return',
    start: `/logowanie?next=${encodeURIComponent(`/admin/zamowienia/${id}`)}`,
    target: '/konto/zamowienia',
    role: 'client',
  },
  {
    name: 'email order link',
    start: `/konto/zamowienia/${id}#platnosc`,
    target: `/konto/zamowienia/${id}#platnosc`,
  },
  {
    name: 'explicit order return',
    start: `/logowanie?next=${encodeURIComponent(`/konto/zamowienia/${id}`)}`,
    target: `/konto/zamowienia/${id}`,
  },
  { name: 'guest checkout return', start: '/logowanie?next=%2Fzamowienie', target: '/zamowienie' },
  {
    name: 'external redirect rejection',
    start: '/logowanie?next=https%3A%2F%2Fexample.invalid%2Fsteal',
    target: '/konto/zamowienia',
  },
]) {
  test(`login preserves ${scenario.name} without accepting arbitrary destinations`, async ({
    page,
  }) => {
    const api = await fixture(page, scenario.role ?? 'client');
    api.signOut();
    await page.goto(scenario.start);
    await page.getByLabel('Adres email').fill('client@example.invalid');
    await page.getByLabel('Hasło', { exact: true }).fill('correct horse battery');
    await page.getByRole('button', { name: 'Zaloguj hasłem', exact: true }).click();
    await page.getByLabel('Kod z emaila').fill('12345678');
    await page.getByRole('button', { name: 'Potwierdź i zaloguj', exact: true }).click();
    await expect(page).toHaveURL(new URL(scenario.target, 'http://127.0.0.1:18431').href);
    if (scenario.target.includes(id))
      await expect(page.getByRole('heading', { name: 'Rozmowa o zamówieniu' })).toBeVisible();
    expect(api.writes.find((w) => w.path.endsWith('/claim-guest'))).toBeDefined();
  });
}
