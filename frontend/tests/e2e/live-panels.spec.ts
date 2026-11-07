import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
const id = '00000000-0000-4000-8000-000000000001';
async function fixture(
  page: Page,
  role = 'client',
  verified = true,
  lineTotal = 52,
  preview = false,
) {
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
    lines: [{ model_id: 'model-1', quantity: 3, unit_grosze: 18, total_grosze: lineTotal }],
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
            preview_url: preview ? `${path}/model-1/preview.glb` : undefined,
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

test('order preview redraws after native WebGL context restoration without interaction', async ({
  page,
}, info) => {
  await fixture(page, 'client', true, 52, true);
  // Canonical wire fixture using the checked-in cube's real positions/indices.
  // The source GLB is COLLADA2GLTF; it is not a backend private-preview response.
  const source = readFileSync('../backend/crates/geometry/tests/fixtures/simple.glb');
  const sourceJsonLength = source.readUInt32LE(12);
  const sourceJson = JSON.parse(source.subarray(20, 20 + sourceJsonLength).toString());
  const binaryStart = 28 + sourceJsonLength;
  const position = sourceJson.accessors[2],
    index = sourceJson.accessors[0];
  const bytes = (accessor: typeof position) => {
    const view = sourceJson.bufferViews[accessor.bufferView];
    const start = binaryStart + (view.byteOffset ?? 0) + (accessor.byteOffset ?? 0);
    return source.subarray(start, start + accessor.count * (accessor.type === 'VEC3' ? 12 : 2));
  };
  const positions = bytes(position),
    indices = bytes(index);
  const binary = Buffer.concat([positions, indices]);
  const json = Buffer.from(
    JSON.stringify({
      asset: { version: '2.0', generator: 'rapidmaker-geometry' },
      buffers: [{ byteLength: binary.length }],
      bufferViews: [
        { buffer: 0, byteOffset: 0, byteLength: positions.length },
        { buffer: 0, byteOffset: positions.length, byteLength: indices.length },
      ],
      accessors: [
        { bufferView: 0, componentType: 5126, count: position.count, type: 'VEC3' },
        { bufferView: 1, componentType: 5123, count: index.count, type: 'SCALAR' },
      ],
      meshes: [{ primitives: [{ attributes: { POSITION: 0 }, indices: 1, mode: 4 }] }],
      nodes: [{ mesh: 0, scale: [0.001, 0.001, 0.001], rotation: [-0.70710678, 0, 0, 0.70710678] }],
      scenes: [{ nodes: [0] }],
      scene: 0,
    }),
  );
  const padded = Buffer.concat([json, Buffer.alloc((4 - (json.length % 4)) % 4, 0x20)]);
  const body = Buffer.alloc(28 + padded.length + binary.length);
  [0x46546c67, 2, body.length, padded.length, 0x4e4f534a].forEach((value, offset) =>
    body.writeUInt32LE(value, offset * 4),
  );
  padded.copy(body, 20);
  body.writeUInt32LE(binary.length, 20 + padded.length);
  body.writeUInt32LE(0x004e4942, 24 + padded.length);
  binary.copy(body, 28 + padded.length);
  await page.route(`**/api/account/orders/${id}/files/model-1/preview.glb`, (route) =>
    route.fulfill({ contentType: 'model/gltf-binary', body }),
  );
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto(`/konto/zamowienia/${id}`);
  const toggle = page.getByRole('button', { name: /podgląd.*uchwyt\.stl/ });
  if ((await toggle.getAttribute('aria-expanded')) !== 'true') await toggle.click();
  const canvas = page.locator('.model-preview canvas');
  await expect(page.locator('.model-preview')).toHaveAttribute('aria-busy', 'false');
  await expect(page.getByRole('button', { name: 'Obróć model w prawo' })).toBeVisible();
  const extension = await canvas.evaluateHandle((node: HTMLCanvasElement) => {
    const counts = { lost: 0, restored: 0, resized: 0, trusted: true };
    Object.assign(node, { previewEvents: counts });
    node.addEventListener('webglcontextlost', (event) => {
      counts.lost++;
      counts.trusted &&= event.isTrusted;
    });
    node.addEventListener('webglcontextrestored', (event) => {
      counts.restored++;
      counts.trusted &&= event.isTrusted;
    });
    new ResizeObserver(() => counts.resized++).observe(node);
    return node.getContext('webgl')?.getExtension('WEBGL_lose_context');
  });
  expect(await extension.evaluate((value) => !!value)).toBe(true);
  const events = () =>
    canvas.evaluate(
      (node) =>
        (
          node as HTMLCanvasElement & {
            previewEvents: { lost: number; restored: number; resized: number; trusted: boolean };
          }
        ).previewEvents,
    );
  await expect.poll(async () => (await events()).resized).toBeGreaterThan(0);
  const silhouette = async (name: string) => {
    const png = await canvas.screenshot({
      path: info.outputPath(`${name}.png`),
      style: '.model-preview canvas { background: #101010 !important; }',
    });
    return page.evaluate(async (base64) => {
      const image = new Image();
      image.src = `data:image/png;base64,${base64}`;
      await image.decode();
      const probe = document.createElement('canvas');
      probe.width = probe.height = 96;
      const context = probe.getContext('2d')!;
      context.drawImage(image, 0, 0, 96, 96);
      const pixels = context.getImageData(0, 0, 96, 96).data;
      return Array.from({ length: 96 * 96 }, (_, index) => {
        const [r, g, b] = pixels.slice(index * 4, index * 4 + 3);
        return Math.min(r, g, b) > 80 && Math.max(r, g, b) - Math.min(r, g, b) < 45 ? 1 : 0;
      });
    }, png.toString('base64'));
  };
  const metrics: unknown[] = [];
  try {
    let before = await silhouette('initial');
    expect(before.reduce((sum, pixel) => sum + pixel, 0)).toBeGreaterThan(100);
    for (let cycle = 1; cycle <= 2; cycle++) {
      const resized = (await events()).resized;
      await extension.evaluate((value) => value!.loseContext());
      await expect.poll(async () => (await events()).lost).toBe(cycle);
      await extension.evaluate((value) => value!.restoreContext());
      await expect.poll(async () => (await events()).restored).toBe(cycle);
      const restored = await silhouette(`restored-${cycle}`);
      const pixels = restored.reduce((sum, pixel) => sum + pixel, 0);
      metrics.push({
        cycle,
        beforePixels: before.reduce((sum, pixel) => sum + pixel, 0),
        pixels,
        events: await events(),
      });
      expect(await events()).toEqual({ lost: cycle, restored: cycle, resized, trusted: true });
      expect(
        pixels,
        'Restored white mesh is visible before any resize or rotation',
      ).toBeGreaterThan(100);
      expect(restored.filter((pixel, index) => pixel !== before[index]).length).toBeLessThan(100);
      await page.getByRole('button', { name: 'Obróć model w prawo' }).click();
      const rotated = await silhouette(`rotated-${cycle}`);
      expect(rotated.filter((pixel, index) => pixel !== restored[index]).length).toBeGreaterThan(
        20,
      );
      before = rotated;
    }
    await toggle.click();
    await expect(canvas).toHaveCount(0);
    expect(errors).toEqual([]);
  } finally {
    await info.attach('native-context-pixels', {
      body: JSON.stringify({ metrics, errors }),
      contentType: 'application/json',
    });
    await extension.dispose();
  }
});

test('failed order preview terminates its native worker while still expanded', async ({ page }) => {
  await fixture(page, 'client', true, 52, true);
  let failures = 0;
  await page.route(`**/api/account/orders/${id}/files/model-1/preview.glb`, (route) => {
    failures++;
    return route.fulfill({ status: 503, body: 'Preview unavailable' });
  });
  await page.addInitScript(() => {
    const NativeWorker = window.Worker;
    const live = new Set<Worker>();
    const counts = {
      created: 0,
      terminated: 0,
      get live() {
        return live.size;
      },
    };
    Object.assign(window, {
      previewWorkers: counts,
      Worker: class extends NativeWorker {
        constructor(...args: ConstructorParameters<typeof Worker>) {
          super(...args);
          live.add(this);
          counts.created++;
        }
        terminate() {
          super.terminate();
          if (live.delete(this)) counts.terminated++;
        }
      },
    });
  });
  await page.goto(`/konto/zamowienia/${id}`);
  const toggle = page.getByRole('button', { name: /podgląd.*uchwyt\.stl/ });
  for (let attempt = 1; attempt <= 2; attempt++) {
    if (attempt > 1) await toggle.click();
    await expect(
      page.getByText('Podgląd jest niedostępny. Oryginał możesz pobrać poniżej.', { exact: true }),
    ).toBeVisible();
    await expect(toggle).toHaveAttribute('aria-expanded', 'true');
    expect(failures).toBe(attempt);
    expect(
      await page.evaluate(() => {
        const counts = (
          window as unknown as Window & {
            previewWorkers: { created: number; terminated: number; live: number };
          }
        ).previewWorkers;
        return { created: counts.created, terminated: counts.terminated, live: counts.live };
      }),
    ).toEqual({ created: attempt, terminated: attempt, live: 0 });
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-expanded', 'false');
  }
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

for (const total of [52, 54]) {
  test(`admin unit price identifies rounding for line total ${total}`, async ({ page }) => {
    const api = await fixture(page, 'admin', true, total);
    await page.goto('/admin.html');
    await page.getByRole('button', { name: `Otwórz ${id}`, exact: true }).click();
    const unit = page.getByLabel(
      total === 52 ? 'Cena / szt. (zł, orientacyjnie)' : 'Cena / szt. (zł)',
      { exact: true },
    );
    await expect(unit).toBeDisabled();
    await expect(unit).toHaveValue('0.18');
    await expect(page.locator('.model-controls strong')).toContainText(
      total === 52 ? '0,52' : '0,54',
    );
    expect(api.writes).toHaveLength(0);
  });
}

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
