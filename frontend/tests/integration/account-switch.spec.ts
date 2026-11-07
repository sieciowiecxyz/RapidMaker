import { expect, test, type Page } from './fixtures';
import { accounts, login, type Account } from './helpers';
import { randomUUID } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Real password/TOTP sessions; never capture credentials, cookies or auth forms.
test.use({ screenshot: 'off', trace: 'off' });

async function headers(page: Page) {
  const me = await (await page.request.get('/api/auth/me')).json();
  expect(me.stage).toBe('authenticated');
  return { Origin: 'http://localhost:18437', 'X-CSRF-Token': me.csrf_token };
}

async function createOrder(page: Page, account: Account, label: string) {
  const filename = `${label}.stl`;
  const message = `Prywatna rozmowa ${label}`;
  const upload = await page.request.post('/api/models', {
    headers: {
      ...(await headers(page)),
      'Content-Type': 'application/octet-stream',
      'x-model-filename': filename,
    },
    data: readFileSync(resolve('../backend/crates/geometry/tests/fixtures/huge_cube_1000mm.stl')),
  });
  expect(upload.status()).toBe(201);
  const model = await upload.json();
  const catalog = await (await page.request.get('/api/catalog/materials')).json();
  const color = catalog.materials.find((item: { id: string }) => item.id === 'GFA00').colors[0]
    .code;
  const created = await page.request.post('/api/account/manual-quotes', {
    headers: await headers(page),
    data: {
      request_id: randomUUID(),
      parts: [
        {
          model_id: model.id,
          quantity: 1,
          material_id: 'GFA00',
          color_code: color,
          options: { quality: 'standard', wall_loops: 3, infill_percent: 20 },
        },
      ],
      contact: {
        name: label,
        email: account.email,
        phone: '500600700',
        country: 'PL',
        street: '',
        apartment: '',
        city: '',
        postal_code: '',
        invoice: null,
      },
      delivery: { method: 'pickup' },
      notes: `Prywatny zakres ${label}`,
    },
  });
  expect(created.status()).toBe(200);
  const { order_id: id } = await created.json();
  const api = `/api/account/orders/${id}`;
  const order = await page.request.get(api);
  expect(order.status()).toBe(200);
  expect((await order.json()).manual_offer.state).toBe('awaiting_quote');
  const files = await page.request.get(`${api}/files`);
  expect(files.status()).toBe(200);
  const file = (await files.json()).files[0];
  expect(file).toMatchObject({ model_id: model.id, filename, available: true });
  expect(file.source_url).toBeTruthy();
  expect(file.preview_url).toBeTruthy();
  const privatePaths = [
    api,
    `${api}/messages`,
    `${api}/files`,
    file.source_url as string,
    file.preview_url as string,
    `/api/models/${model.id}`,
    `/api/models/${model.id}/preview.glb`,
  ];
  // Prove the protected artifacts exist and are readable by their actual owner first.
  for (const path of privatePaths) expect((await page.request.get(path)).status()).toBe(200);
  await page.goto(`/konto/zamowienia/${id}#rozmowa`);
  const number = `RM-${id.slice(0, 8).toUpperCase()}`;
  await expect(page.getByRole('heading', { name: number, exact: true })).toBeVisible();
  await expect(page.getByText(filename, { exact: true }).first()).toBeVisible();
  await page.getByLabel('Wiadomość', { exact: true }).fill(message);
  await page.getByRole('button', { name: 'Wyślij wiadomość', exact: true }).click();
  await expect(page.getByText(message, { exact: true })).toBeVisible();
  return { id, number, filename, message, label, privatePaths };
}

test('UI logout A then login B in the same tab isolates orders, chat and private model history', async ({
  page,
  context,
  browser,
}, info) => {
  test.skip(
    !process.env.RAPIDMAKER_TEST_ACCOUNTS || process.env.RAPIDMAKER_SANDBOX === '1',
    'Requires --local-accounts with providers and mail disabled',
  );
  test.setTimeout(180_000);
  const clientA = accounts().find((account) => account.email === 'browser-other@example.invalid')!;
  const clientB = accounts().find(
    (account) => account.email === 'browser-security@example.invalid',
  )!;
  expect(Boolean(clientA.totp_uri && clientB.totp_uri)).toBe(true);
  const suffix = randomUUID();
  // Independent B setup creates a genuine owned order. The actual regression reuses A's
  // page, cookie jar and history through logout/login, without storage or cookie clearing.
  const setup = await browser.newContext();
  let orderB: Awaited<ReturnType<typeof createOrder>>;
  try {
    const seed = await setup.newPage();
    await login(seed, clientB);
    orderB = await createOrder(seed, clientB, `switch-B-${suffix}`);
  } finally {
    await setup.close();
  }
  await login(page, clientA);
  await page.getByRole('link', { name: 'Ustawienia konta', exact: true }).click();
  await expect(page.getByText(clientA.email, { exact: true })).toBeVisible();
  const orderA = await createOrder(page, clientA, `switch-A-${suffix}`);
  const oldUrl = page.url();
  const oldHistoryLength = await page.evaluate(() => history.length);
  const documentRequests: string[] = [];
  page.on('request', (request) => {
    if (request.isNavigationRequest() && request.frame() === page.mainFrame())
      documentRequests.push(new URL(request.url()).pathname);
  });
  const loggedOut = page.waitForResponse((r) => r.url().endsWith('/api/auth/logout'));
  await page.getByRole('button', { name: 'Wyloguj', exact: true }).click();
  expect((await loggedOut).status()).toBe(200);
  await expect(page).toHaveURL('http://localhost:18437/');
  expect((await page.request.get('/api/auth/me')).status()).toBe(401);
  await login(page, clientB);
  expect(page.context()).toBe(context);
  const me = await (await page.request.get('/api/auth/me')).json();
  expect(me.user.id === clientB.id).toBe(true);
  const noA = async () => {
    for (const text of [
      orderA.number,
      orderA.filename,
      orderA.message,
      orderA.label,
      clientA.email,
    ])
      await expect(page.getByText(text, { exact: false })).toHaveCount(0);
  };
  await noA();
  await expect(page.getByText(orderB.number, { exact: true })).toBeVisible();
  for (const path of orderA.privatePaths)
    expect((await page.request.get(path)).status(), `B cannot read A: ${path}`).toBe(404);
  for (const path of orderB.privatePaths)
    expect((await page.request.get(path)).status(), `B can read own: ${path}`).toBe(200);
  await page.getByRole('link', { name: 'Ustawienia konta', exact: true }).click();
  await expect(page.getByText(clientB.email, { exact: true })).toBeVisible();
  await noA();
  await page.getByRole('link', { name: 'Zamówienia', exact: true }).click();
  // Follow B's real list link, retaining the old private A history entry.
  await page.getByRole('link', { name: new RegExp(orderB.number) }).click();
  await expect(page).toHaveURL(new RegExp(`/konto/zamowienia/${orderB.id}$`));
  await expect(page.getByText(orderB.message, { exact: true })).toBeVisible();
  await expect(page.getByText(orderB.filename, { exact: true }).first()).toBeVisible();
  await noA();
  const distance = (await page.evaluate(() => history.length)) - oldHistoryLength;
  expect(distance).toBeGreaterThan(0);
  await page.evaluate((steps) => history.go(-steps), distance);
  await expect(page).toHaveURL(oldUrl);
  await expect(page.getByRole('heading', { name: 'Nie można otworzyć zamówienia' })).toBeVisible();
  await noA();
  const afterHistory = await page.request.get('/api/auth/me');
  expect(afterHistory.status()).toBe(200);
  expect((await afterHistory.json()).user.id === clientB.id).toBe(true);
  expect((await page.request.get(`/api/account/orders/${orderB.id}`)).status()).toBe(200);
  // Logout intentionally navigates the document; this checks the old private URL under B,
  // without claiming BFCache restoration or classifying browser-local drafts as account data.
  expect(documentRequests).toContain('/');
  await info.attach('same-tab-history.json', {
    body: JSON.stringify({
      samePageAndContext: true,
      logoutDocumentNavigation: true,
      traversedEntries: distance,
      oldPrivateAnchorReached: new URL(page.url()).hash === '#rozmowa',
      oldOrderDenied: true,
      bOwnOrderReadable: true,
      bfcache: 'not asserted',
    }),
    contentType: 'application/json',
  });
});

test('cross-tab cookie changes remove private A detail and chat before B can use the old route', async ({
  page,
  context,
  browser,
}, info) => {
  test.skip(
    !process.env.RAPIDMAKER_TEST_ACCOUNTS || process.env.RAPIDMAKER_SANDBOX === '1',
    'Requires --local-accounts',
  );
  test.setTimeout(180_000);
  const clientA = accounts().find((a) => a.email === 'browser-other@example.invalid')!;
  const clientB = accounts().find((a) => a.email === 'browser-security@example.invalid')!;
  const setup = await browser.newContext();
  let orderB: Awaited<ReturnType<typeof createOrder>>;
  try {
    const seed = await setup.newPage();
    await login(seed, clientB);
    orderB = await createOrder(seed, clientB, `tabs-B-${randomUUID()}`);
  } finally {
    await setup.close();
  }
  await login(page, clientA);
  const orderA = await createOrder(page, clientA, `tabs-A-${randomUUID()}`);
  const observer = await context.newPage();
  await observer.goto(`/konto/zamowienia/${orderA.id}#rozmowa`);
  await expect(observer.getByText(orderA.message, { exact: true })).toBeVisible();
  await expect(observer.getByText(orderA.filename, { exact: true }).first()).toBeVisible();
  // Rejected credentials do not replace the cookie or invalidate the other view.
  await page.goto('/logowanie?reauth=1');
  const signalBefore = await page.evaluate(() => localStorage.getItem('rapidmaker:auth-change'));
  await page.getByLabel('Adres email', { exact: true }).fill(clientA.email);
  await page.getByLabel('Hasło', { exact: true }).fill('deliberately-wrong-password');
  const rejected = page.waitForResponse((r) => r.url().endsWith('/api/auth/password/login'));
  await page.getByRole('button', { name: 'Zaloguj hasłem', exact: true }).click();
  expect((await rejected).status()).toBe(401);
  expect(await page.evaluate(() => localStorage.getItem('rapidmaker:auth-change'))).toBe(
    signalBefore,
  );
  await expect(observer.getByText(orderA.message, { exact: true })).toBeVisible();
  // Public browser-local drafts are deliberately unrelated to account credentials.
  const draft = JSON.stringify({
    models: [{ id: 'local-example', name: 'Lokalny przykład', quantity: 3 }],
    delivery: 'pickup',
  });
  await observer.evaluate(
    (value) => localStorage.setItem('rapidmaker.storefront.draft.v1', value),
    draft,
  );
  // Delay an actual A response from the chat's reload until B is authenticated.
  // Nothing is synthesized: route.fetch obtains the authorized 200 before logout.
  let release!: () => void;
  let captured!: () => void;
  let delivered!: () => void;
  const held = new Promise<void>((resolve) => {
    captured = resolve;
  });
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const done = new Promise<void>((resolve) => {
    delivered = resolve;
  });
  let intercepted = false;
  await observer.route(`**/api/account/orders/${orderA.id}`, async (route) => {
    if (intercepted || route.request().method() !== 'GET') return route.continue();
    intercepted = true;
    const response = await route.fetch();
    expect(response.status()).toBe(200);
    captured();
    await gate;
    await route.fulfill({ response });
    delivered();
  });
  await observer.getByLabel('Wiadomość', { exact: true }).fill(`Spóźniony odczyt ${orderA.label}`);
  await observer.getByRole('button', { name: 'Wyślij wiadomość', exact: true }).click();
  await held;
  try {
    const loggedOut = page.waitForResponse((r) => r.url().endsWith('/api/auth/logout'));
    await page.getByRole('button', { name: 'Wyloguj', exact: true }).click();
    expect((await loggedOut).status()).toBe(200);
    expect((await observer.request.get('/api/auth/me')).status()).toBe(401);
    await info.attach('cross-tab-logout-observation.json', {
      body: JSON.stringify({
        sharedCookieLoggedOut: true,
        oldPrivateChatStillVisible: await observer
          .getByText(orderA.message, { exact: true })
          .isVisible(),
      }),
      contentType: 'application/json',
    });
    await expect(observer.getByText(orderA.message, { exact: true })).toHaveCount(0);
    await expect(observer.getByText(orderA.number, { exact: true })).toHaveCount(0);
    await observer.evaluate(
      (forbidden) => {
        const probe = { leaked: false };
        Object.assign(window, { privateViewProbe: probe });
        new MutationObserver(() => {
          if (forbidden.some((value) => document.body.innerText.includes(value)))
            probe.leaked = true;
        }).observe(document.body, { childList: true, subtree: true, characterData: true });
      },
      [orderA.message, orderA.filename, orderA.number, orderA.label, clientA.email],
    );
    await login(page, clientB);
    const noA = async () => {
      for (const text of [
        orderA.message,
        orderA.filename,
        orderA.number,
        orderA.label,
        clientA.email,
      ])
        await expect(observer.getByText(text, { exact: false })).toHaveCount(0);
    };
    await expect(
      observer.getByRole('heading', { name: 'Nie można otworzyć zamówienia' }),
    ).toBeVisible();
    release();
    await done;
    await noA();
    expect(
      await observer.evaluate(
        () =>
          (window as unknown as { privateViewProbe: { leaked: boolean } }).privateViewProbe.leaked,
      ),
    ).toBe(false);
    for (const path of orderA.privatePaths)
      expect((await observer.request.get(path)).status()).toBe(404);
    await observer.goto(`/konto/zamowienia/${orderB.id}#rozmowa`);
    await expect(observer.getByText(orderB.message, { exact: true })).toBeVisible();
    await expect(observer.getByText(orderB.filename, { exact: true }).first()).toBeVisible();
    await noA();
    expect(
      await observer.evaluate(() => localStorage.getItem('rapidmaker.storefront.draft.v1')),
    ).toBe(draft);
  } finally {
    release();
  }
});

test('cross-tab logout removes protected admin selection and chat when a client signs in', async ({
  page,
  context,
  browser,
}) => {
  test.skip(
    !process.env.RAPIDMAKER_TEST_ACCOUNTS || process.env.RAPIDMAKER_SANDBOX === '1',
    'Requires --local-accounts',
  );
  test.setTimeout(180_000);
  const client = accounts().find((a) => a.email === 'browser-other@example.invalid')!;
  const admin = accounts().find((a) => a.role === 'admin')!;
  const setup = await browser.newContext();
  let order: Awaited<ReturnType<typeof createOrder>>;
  try {
    const seed = await setup.newPage();
    await login(seed, client);
    order = await createOrder(seed, client, `tabs-admin-${randomUUID()}`);
  } finally {
    await setup.close();
  }
  await login(page, admin);
  const observer = await context.newPage();
  await observer.goto(`/admin.html?order=${order.id}`);
  await expect(observer.getByText(order.filename, { exact: true }).first()).toBeVisible();
  await observer
    .getByRole('complementary', { name: /^Szczegóły / })
    .getByRole('button', { name: 'Wiadomości', exact: true })
    .click();
  await expect(observer.getByText(order.message, { exact: true })).toBeVisible();
  const loggedOut = page.waitForResponse((r) => r.url().endsWith('/api/auth/logout'));
  await page.getByRole('button', { name: 'Wyloguj', exact: true }).click();
  expect((await loggedOut).status()).toBe(200);
  await expect(observer.getByText(order.message, { exact: true })).toHaveCount(0);
  await expect(
    observer.getByRole('button', { name: 'Zamknij szczegóły', exact: true }),
  ).toHaveCount(0);
  await login(page, client);
  expect((await observer.request.get('/api/admin/orders')).status()).toBe(403);
  await expect(
    observer.getByRole('button', { name: 'Zamknij szczegóły', exact: true }),
  ).toHaveCount(0);
  await observer.goto(`/konto/zamowienia/${order.id}#rozmowa`);
  await expect(observer.getByText(order.message, { exact: true })).toBeVisible();
});
