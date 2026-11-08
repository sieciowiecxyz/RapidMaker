import { expect, test, type Page } from './fixtures';
import { accounts, login } from './helpers';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { fillAuthorContact } from './checkout-helpers';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Authentication is real; no credentials or session/payment secrets are captured.
test.use({ screenshot: 'off', trace: 'off' });

async function headers(page: Page) {
  const me = await (await page.request.get('/api/auth/me')).json();
  expect(me.stage).toBe('authenticated');
  return { Origin: 'http://localhost:18437', 'X-CSRF-Token': me.csrf_token };
}

function inspectOwned(id: string, query: string) {
  const database = process.env.RAPIDMAKER_TEST_DATABASE ?? '';
  if (
    !/^rapidmaker_test_browser_[a-f0-9]+$/.test(database) ||
    !/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(id)
  )
    throw new Error('Inspection requires an isolated database and owned resource UUID');
  return JSON.parse(
    execFileSync(
      'podman',
      [
        'exec',
        process.env.TEST_PG_CONTAINER ?? 'rapidmaker-catalog-postgres',
        'psql',
        '-U',
        'rapidmaker',
        '-d',
        database,
        '-v',
        'ON_ERROR_STOP=1',
        '-At',
        '-c',
        query,
      ],
      { encoding: 'utf8' },
    ).trim(),
  );
}

function offerEmails(id: string) {
  return inspectOwned(
    id,
    `SELECT COALESCE(json_agg(json_build_object('event_key',event_key,'status',status,'attempts',attempts) ORDER BY event_key),'[]'::json) FROM email_outbox WHERE kind='manual_offer' AND payload->'notification'->>'reference'='${id}'`,
  );
}

test('manual publication recovers before and after commit without duplicate offers or email', async ({
  page,
  browser,
}) => {
  test.skip(
    !process.env.RAPIDMAKER_TEST_ACCOUNTS || process.env.RAPIDMAKER_SANDBOX === '1',
    'Requires --local-accounts with providers disabled',
  );
  test.setTimeout(180_000);
  const client = accounts().find((account) => account.email === 'browser-other@example.invalid')!;
  const admin = accounts().find((account) => account.role === 'admin')!;
  const adminContext = await browser.newContext();
  const operator = await adminContext.newPage();
  const errors: string[] = [];
  operator.on('pageerror', (error) => errors.push(error.name));
  try {
    await login(page, client);
    await login(operator, admin);
    const upload = await page.request.post('/api/models', {
      headers: {
        ...(await headers(page)),
        'Content-Type': 'application/octet-stream',
        'x-model-filename': 'cube_20mm.stl',
      },
      data: readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl')),
    });
    expect(upload.status()).toBe(201);
    const model = await upload.json();
    const catalog = await (await operator.request.get('/api/admin/materials')).json();
    const material = catalog.materials.find((item: { id: string }) => item.id === 'GFA00');
    const created = await page.request.post('/api/account/manual-quotes', {
      headers: await headers(page),
      data: {
        request_id: randomUUID(),
        parts: [
          {
            model_id: model.id,
            quantity: 3,
            material_id: 'GFA00',
            color_code: material.colors[0].code,
            options: { quality: 'standard', wall_loops: 3, infill_percent: 20 },
          },
        ],
        contact: {
          name: 'Test odzyskiwania publikacji',
          email: client.email,
          phone: '500600700',
          country: 'PL',
          street: '',
          apartment: '',
          city: '',
          postal_code: '',
          invoice: null,
        },
        delivery: { method: 'pickup' },
        notes: 'Test przerwanego połączenia przy publikacji oferty.',
      },
    });
    expect(created.status()).toBe(200);
    const { order_id: id } = await created.json();
    const readOffer = async () => {
      const response = await page.request.get(`/api/account/orders/${id}`);
      expect(response.status()).toBe(200);
      return (await response.json()).manual_offer;
    };
    expect(await readOffer()).toMatchObject({ state: 'awaiting_quote', version: 1 });
    expect(offerEmails(id)).toEqual([]);
    await operator.goto(`/admin/zamowienia/${id}`);
    const offer = operator.getByRole('region', { name: 'Wycena indywidualna' });
    const total = offer.getByLabel('Całość brutto (zł)');
    const shipping = offer.getByLabel('W tym dostawa (zł)');
    const scope = offer.getByLabel('Zakres wykonania i dostawy');
    const scopeText = 'Trzy części PLA, odbiór osobisty, realizacja do 7 dni.';
    await total.fill('25,00');
    await shipping.fill('0,00');
    await scope.fill(scopeText);
    const quoteUrl = `http://localhost:18437/api/admin/orders/${id}/quote`;
    const attempts: unknown[] = [];
    operator.on('request', (request) => {
      if (request.url() === quoteUrl && request.method() === 'POST')
        attempts.push(request.postDataJSON());
    });
    await operator.route(quoteUrl, async (route) => {
      expect(route.request().method()).toBe('POST');
      // No fetch: the first publication must never reach the API.
      await route.abort('connectionfailed');
    });
    operator.once('dialog', async (dialog) => {
      expect(dialog.type()).toBe('confirm');
      await dialog.accept();
    });
    await offer.getByRole('button', { name: 'Opublikuj ofertę i powiadom klienta' }).click();
    await expect(offer.getByRole('alert')).toBeVisible();
    await expect(total).toHaveValue('25,00');
    await expect(shipping).toHaveValue('0,00');
    await expect(scope).toHaveValue(scopeText);
    expect(await readOffer()).toMatchObject({ state: 'awaiting_quote', version: 1 });
    expect(offerEmails(id)).toEqual([]);
    const retry = offer.getByRole('button', { name: 'Sprawdź / ponów zapis oferty' });
    await expect(retry).toBeVisible();
    await expect(retry).toBeEnabled();
    await operator.unroute(quoteUrl);
    const unexpectedDialog = async (dialog: import('@playwright/test').Dialog) => {
      await dialog.dismiss();
      throw new Error('Retry must preserve the confirmed publication without asking again');
    };
    operator.on('dialog', unexpectedDialog);
    const saved = operator.waitForResponse(
      (response) => response.url() === quoteUrl && response.request().method() === 'POST',
    );
    await retry.click();
    expect((await saved).status()).toBe(200);
    operator.off('dialog', unexpectedDialog);
    expect(attempts).toHaveLength(2);
    expect(attempts[1]).toEqual(attempts[0]);
    await expect.poll(readOffer).toMatchObject({
      state: 'offered',
      version: 2,
      amount_grosze: 2500,
      shipping_grosze: 0,
      scope: scopeText,
    });
    expect(offerEmails(id)).toEqual([
      { event_key: `order:${id}:manual-offer:2`, status: 'pending', attempts: 0 },
    ]);
    await expect(total).toBeEnabled();
    await total.fill('30,00');
    let committed = false;
    await operator.route(quoteUrl, async (route) => {
      expect(route.request().method()).toBe('POST');
      const response = await route.fetch();
      expect(response.status()).toBe(200);
      committed = true;
      // The API committed the real mutation; only its response is lost.
      await route.abort('connectionfailed');
    });
    operator.once('dialog', async (dialog) => {
      expect(dialog.type()).toBe('confirm');
      await dialog.accept();
    });
    await offer.getByRole('button', { name: 'Opublikuj ofertę i powiadom klienta' }).click();
    await expect.poll(() => committed).toBe(true);
    await expect.poll(readOffer).toMatchObject({
      state: 'offered',
      version: 3,
      amount_grosze: 3000,
      shipping_grosze: 0,
      scope: scopeText,
    });
    await expect(offer.getByRole('status')).toHaveText(
      'Oferta zapisana i przekazana do wysyłki email.',
    );
    await expect(offer.getByRole('alert')).toHaveCount(0);
    await expect(total).toBeEnabled();
    await expect(total).toHaveValue('30,00');
    await expect(shipping).toBeEnabled();
    await expect(scope).toBeEnabled();
    await expect(scope).toHaveValue(scopeText);
    await expect(retry).toHaveCount(0);
    expect(attempts).toHaveLength(3); // Recovery reads the committed state, without another POST.
    expect(offerEmails(id)).toEqual([
      { event_key: `order:${id}:manual-offer:2`, status: 'skipped', attempts: 0 },
      { event_key: `order:${id}:manual-offer:3`, status: 'pending', attempts: 0 },
    ]);
    // Two independently authenticated panel sessions publish different prices from version 3.
    const rivalContext = await browser.newContext();
    try {
      const rival = await rivalContext.newPage();
      rival.on('pageerror', (error) => errors.push(error.name));
      await login(rival, admin);
      await rival.goto(`/admin/zamowienia/${id}`);
      const rivalOffer = rival.getByRole('region', { name: 'Wycena indywidualna' });
      await expect(rivalOffer.getByLabel('Całość brutto (zł)')).toHaveValue('30,00');
      await operator.unroute(quoteUrl);
      await total.fill('35,00');
      await rivalOffer.getByLabel('Całość brutto (zł)').fill('40,00');
      for (const target of [operator, rival]) target.once('dialog', (dialog) => dialog.accept());
      const responses = [operator, rival].map((target) =>
        target.waitForResponse(
          (response) => response.url() === quoteUrl && response.request().method() === 'POST',
        ),
      );
      await Promise.all(
        [offer, rivalOffer].map((card) =>
          card.getByRole('button', { name: 'Opublikuj ofertę i powiadom klienta' }).click(),
        ),
      );
      const results = await Promise.all(responses);
      expect(results.map((response) => response.status()).sort()).toEqual([200, 409]);
      const winner = results[0].status() === 200 ? 3500 : 4000;
      await expect
        .poll(readOffer)
        .toMatchObject({ version: 4, amount_grosze: winner, scope: scopeText });
      for (const card of [offer, rivalOffer]) {
        await expect(card.getByLabel('Całość brutto (zł)')).toHaveValue(
          (winner / 100).toFixed(2).replace('.', ','),
        );
        await expect(card.getByLabel('Całość brutto (zł)')).toBeEnabled();
        await expect(card.getByRole('alert')).toHaveCount(0);
      }
      const loser = results[0].status() === 409 ? offer : rivalOffer;
      await expect(loser.getByRole('status')).toHaveText(
        'Oferta zmieniła się na innym urządzeniu. Sprawdź aktualne dane.',
      );
      expect(offerEmails(id)).toEqual([
        { event_key: `order:${id}:manual-offer:2`, status: 'skipped', attempts: 0 },
        { event_key: `order:${id}:manual-offer:3`, status: 'skipped', attempts: 0 },
        { event_key: `order:${id}:manual-offer:4`, status: 'pending', attempts: 0 },
      ]);
    } finally {
      await rivalContext.close();
    }
    const logout = await operator.request.post('/api/auth/logout', {
      headers: await headers(operator),
      data: {},
    });
    expect(logout.status()).toBe(200);
    await offer.getByRole('button', { name: 'Odśwież', exact: true }).click();
    await expect(offer).toHaveCount(0);
    await expect(operator.getByText(scopeText, { exact: true })).toHaveCount(0);
    expect((await operator.request.get('/api/admin/orders')).status()).toBe(401);
    expect(errors).toEqual([]);
  } finally {
    await adminContext.close();
  }
});

test('native terminal failure reaches a persisted manual request through the UI', async ({
  page,
}) => {
  test.skip(
    !process.env.RAPIDMAKER_TEST_ACCOUNTS || process.env.RAPIDMAKER_SANDBOX === '1',
    'Requires local accounts without providers',
  );
  test.setTimeout(150_000);
  const estimates: { status: number; body: unknown }[] = [];
  page.on('response', (response) => {
    if (response.url().endsWith('/api/quotes/estimate') && response.request().method() === 'POST')
      estimates.push({ status: response.status(), body: response.request().postDataJSON() });
  });
  const client = accounts().find((a) => a.email === 'browser-other@example.invalid')!;
  expect(client.totp_uri).toBeTruthy();
  await login(page, client);
  await page.goto('/wycena');
  const uploaded = page.waitForResponse(
    (r) => r.url().endsWith('/api/models') && r.request().method() === 'POST',
  );
  const buffer = Buffer.from(
    readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl')),
  );
  expect(buffer.readUInt32LE(80)).toBe(12);
  // Existing closed cube: -10..10 mm becomes 20 x 20 x 0.01 mm.
  for (let triangle = 0; triangle < 12; triangle++)
    for (let vertex = 0; vertex < 3; vertex++) {
      const z = 84 + triangle * 50 + 12 + vertex * 12 + 8;
      buffer.writeFloatLE((buffer.readFloatLE(z) + 10) * 0.0005, z);
    }
  await page.getByLabel('Dodaj modele STL, STEP lub 3MF').setInputFiles({
    name: 'thin-box_20x20x0.01.stl',
    mimeType: 'application/octet-stream',
    buffer,
  });
  const response = await uploaded;
  expect(response.status()).toBe(201);
  const modelId = () =>
    page.evaluate(
      () =>
        JSON.parse(localStorage.getItem('rapidmaker.storefront.draft.v1') ?? '{}').models?.[0]
          ?.analysis?.id,
    );
  await expect.poll(modelId).toBeTruthy();
  const id = await modelId();
  expect(id).toMatch(/^[a-f0-9-]{36}$/);
  const metadata = await page.request.get(`/api/models/${id}`);
  expect(metadata.status()).toBe(200);
  const model = await metadata.json();
  expect(model.dimensions_mm[0]).toBe(20);
  expect(model.dimensions_mm[1]).toBe(20);
  expect(model.dimensions_mm[2]).toBeCloseTo(0.01, 6);
  await page.locator('input[name=material][value=pla]').check();
  await page.locator('input[name=color]').first().check();
  await page.locator('input[name=quality][value=standard]').check();
  await page.locator('input[name=strength][value=standard]').check();
  await expect(page.getByText('Wymagana ręczna wycena', { exact: true })).toBeVisible({
    timeout: 100_000,
  });
  const jobs = inspectOwned(
    model.id,
    `SELECT json_agg(json_build_object('id',id,'status',status,'attempts',attempts,'lease_cleared',lease_token IS NULL AND lease_until IS NULL)) FROM slicer_jobs WHERE model_id='${model.id}'`,
  );
  expect(jobs).toHaveLength(1);
  expect(jobs[0]).toMatchObject({ status: 'failed', lease_cleared: true });
  expect(jobs[0].attempts).toBeGreaterThanOrEqual(1);
  expect(jobs[0].attempts).toBeLessThanOrEqual(3);
  const job = await (await page.request.get(`/api/quotes/jobs/${jobs[0].id}`)).json();
  expect(job).toMatchObject({ status: 'failed', result: null });
  expect(estimates).toHaveLength(1);
  expect(estimates[0].status).toBe(202);
  await expect(page.locator('.quote-result .price')).toHaveCount(0);
  await page.locator('.quote-contact').click();
  await fillAuthorContact(page);
  await page.getByRole('button', { name: 'Przejdź do indywidualnej wyceny', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Zapytaj o wycenę', exact: true })).toBeVisible();
  await page.getByRole('radio', { name: 'Odbiór osobisty', exact: true }).check();
  const created = page.waitForResponse(
    (r) => r.url().endsWith('/api/account/manual-quotes') && r.request().method() === 'POST',
  );
  await page.getByRole('button', { name: 'Wyślij zapytanie o wycenę', exact: true }).click();
  const sent = await created;
  expect(sent.status()).toBe(200);
  const result = await sent.json();
  expect(result.order_id).toMatch(/^[a-f0-9-]{36}$/);
  const sentBody = sent.request().postDataJSON();
  expect(sentBody.parts).toHaveLength(1);
  const selected = estimates[0].body as { options: Record<string, unknown> } & Record<
    string,
    unknown
  >;
  expect(selected).toEqual({
    model_id: model.id,
    material_id: 'GFA00',
    color_code: '10101',
    options: { quantity: 1, quality: 'standard', wall_loops: 3, infill_percent: 20 },
  });
  const { quantity, ...printOptions } = selected.options;
  expect(quantity).toBe(1);
  // Manual HTTP carries quantity at part level; the API canonicalizes options.
  expect(sentBody.parts[0]).toEqual({ ...selected, quantity, options: printOptions });
  await expect(page).toHaveURL(new RegExp('/konto/zamowienia/' + result.order_id + '$'));
  await expect(
    page.getByRole('heading', { name: 'Oczekuje na wycenę', exact: true }),
  ).toBeVisible();
  const own = await page.request.get(`/api/account/orders/${result.order_id}`);
  expect(own.status()).toBe(200);
  const order = await own.json();
  expect(order.status).toBe('awaiting_quote');
  expect(order.amount_grosze).toBeNull();
  expect(order.manual_offer.payment_started).toBe(false);
  const persisted = inspectOwned(
    result.order_id,
    `SELECT json_build_object('status',o.status,'amount',o.amount_grosze,'payment_attempt',o.payment_attempt_id,'offer',m.offer,'parts',m.input->'parts','hold',EXISTS(SELECT 1 FROM order_files f WHERE f.order_id=o.id AND f.model_id='${model.id}' AND (f.retain_until IS NULL OR f.retain_until>extract(epoch from now())::bigint)),'retain_until',(SELECT max(f.retain_until) FROM order_files f WHERE f.order_id=o.id AND f.model_id='${model.id}'),'payment_attempts',(SELECT count(*) FROM payment_attempts p WHERE p.draft_id=o.id)) FROM orders o JOIN manual_quotes m ON m.order_id=o.id WHERE o.id='${result.order_id}'`,
  );
  expect(persisted).toMatchObject({
    status: 'awaiting_quote',
    amount: null,
    payment_attempt: null,
    offer: null,
    hold: true,
    payment_attempts: 0,
  });
  expect(persisted.parts).toEqual([
    {
      ...sentBody.parts[0],
      requested_color: null,
      options: { ...printOptions, quantity },
    },
  ]);
  expect(estimates).toHaveLength(1);
  await test.info().attach('native-manual-evidence', {
    body: JSON.stringify({ model, jobs, job, persisted, order_id: result.order_id }),
    contentType: 'application/json',
  });
});

test('different manual configurations share one upload and preserve separate order lines', async ({
  page,
  browser,
}) => {
  test.skip(
    !process.env.RAPIDMAKER_TEST_ACCOUNTS || process.env.RAPIDMAKER_SANDBOX === '1',
    'Requires local accounts without providers',
  );
  test.setTimeout(90_000);
  await login(
    page,
    accounts().find((a) => a.email === 'browser-other@example.invalid')!,
  );
  await page.goto('/wycena');
  const buffer = Buffer.from(
    readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl')),
  );
  expect(buffer.readUInt32LE(80)).toBe(12);
  for (let triangle = 0; triangle < 12; triangle++)
    for (let vertex = 0; vertex < 3; vertex++)
      for (let axis = 0; axis < 3; axis++) {
        const offset = 84 + triangle * 50 + 12 + vertex * 12 + axis * 4;
        buffer.writeFloatLE(buffer.readFloatLE(offset) * 15, offset);
      }
  let uploads = 0,
    manualRequests = 0;
  page.on('request', (r) => {
    if (r.url().endsWith('/api/models') && r.method() === 'POST') uploads++;
    if (r.url().endsWith('/api/account/manual-quotes') && r.method() === 'POST') manualRequests++;
  });
  const uploaded = page.waitForResponse(
    (r) => r.url().endsWith('/api/models') && r.request().method() === 'POST',
  );
  await page
    .getByLabel('Dodaj modele STL, STEP lub 3MF')
    .setInputFiles({ name: 'oversized300.stl', mimeType: 'application/octet-stream', buffer });
  expect((await uploaded).status()).toBe(201);
  const draft = () =>
    page.evaluate(
      () => JSON.parse(localStorage.getItem('rapidmaker.storefront.draft.v1') ?? '{}').models,
    );
  await expect.poll(async () => (await draft())?.[0]?.analysis?.id).toBeTruthy();
  const model = (await draft())[0].analysis;
  expect(model.dimensions_mm).toEqual([300, 300, 300]);
  await page.locator('input[name=material][value=pla]').check();
  await page.locator('input[name=color][value=black]').check();
  await page.locator('input[name=quality][value=standard]').check();
  await page.locator('input[name=strength][value=standard]').check();
  await expect(page.getByText('Wymagana ręczna wycena', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Powiel wariant oversized300.stl', exact: true }).click();
  const rows = page.locator('.model-row');
  await expect(rows).toHaveCount(2);
  const identical = page.getByText(
    'Identyczny wariant. Zmień ustawienia lub zwiększ liczbę sztuk.',
    { exact: true },
  );
  await expect(identical).toBeVisible();
  await expect(page.locator('.quote-result .price')).toHaveCount(0);
  await expect(page.locator('.quote-contact')).toBeDisabled();
  expect(manualRequests).toBe(0);
  await rows.nth(1).getByRole('spinbutton').fill('2');
  await expect(page.getByText('Wymagana ręczna wycena', { exact: true })).toBeVisible();
  await expect(page.locator('.quote-contact')).toBeEnabled();
  expect(
    (await draft()).map((m: { settings: { material: string } }) => m.settings.material),
  ).toEqual(['pla', 'pla']);
  expect((await draft()).map((m: { settings: { color: string } }) => m.settings.color)).toEqual([
    'black',
    'black',
  ]);
  await rows.nth(1).getByRole('spinbutton').fill('1');
  await expect(identical).toBeVisible();
  await expect(page.locator('.quote-contact')).toBeDisabled();
  expect(manualRequests).toBe(0);

  await rows
    .nth(1)
    .getByRole('button', { name: 'Konfiguruj oversized300.stl', exact: true })
    .click();
  await page.locator('input[name=material][value=petg]').check();
  await rows.nth(1).getByRole('spinbutton').fill('2');
  expect(new Set((await draft()).map((m: { id: string }) => m.id)).size).toBe(2);
  expect(new Set((await draft()).map((m: { analysis: { id: string } }) => m.analysis.id))).toEqual(
    new Set([model.id]),
  );
  await page.locator('.quote-contact').click();
  await fillAuthorContact(page);
  await page.getByRole('button', { name: 'Przejdź do indywidualnej wyceny', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Zapytaj o wycenę', exact: true })).toBeVisible();
  await page.getByRole('radio', { name: 'Odbiór osobisty', exact: true }).check();
  const created = page.waitForResponse(
    (r) => r.url().endsWith('/api/account/manual-quotes') && r.request().method() === 'POST',
  );
  await page.getByRole('button', { name: 'Wyślij zapytanie o wycenę', exact: true }).click();
  const response = await created;
  expect(response.status()).toBe(200);
  const result = await response.json(),
    body = response.request().postDataJSON();
  const print = { quality: 'standard', wall_loops: 3, infill_percent: 20 };
  const expected = [
    { model_id: model.id, material_id: 'GFA00', color_code: '10101', quantity: 1, options: print },
    { model_id: model.id, material_id: 'GFG02', color_code: '33102', quantity: 2, options: print },
  ];
  expect(body.parts).toEqual(expected);
  await expect(page).toHaveURL(new RegExp('/konto/zamowienia/' + result.order_id + '$'));
  await expect(
    page.getByRole('heading', { name: 'Oczekuje na wycenę', exact: true }),
  ).toBeVisible();
  await expect(page.locator('article.model')).toHaveCount(2);
  const order = await (await page.request.get(`/api/account/orders/${result.order_id}`)).json();
  expect(order).toMatchObject({
    status: 'awaiting_quote',
    amount_grosze: null,
    manual_offer: { payment_started: false },
  });
  expect(new Set(order.lines.map((l: { line_id: string }) => l.line_id)).size).toBe(2);
  expect(order.lines.map((l: { model_id: string }) => l.model_id)).toEqual([model.id, model.id]);
  expect(order.lines.map((l: { quantity: number }) => l.quantity)).toEqual([1, 2]);
  for (const line of order.lines) expect(line.line_id).toBe(line.quote_id);
  const persisted = inspectOwned(
    result.order_id,
    `SELECT json_build_object('parts',m.input->'parts','amount',o.amount_grosze,'offer',m.offer,'payment_attempt',o.payment_attempt_id,'files',(SELECT count(*) FROM order_files f WHERE f.order_id=o.id AND f.model_id='${model.id}' AND (f.retain_until IS NULL OR f.retain_until>extract(epoch from now())::bigint)),'payment_attempts',(SELECT count(*) FROM payment_attempts p WHERE p.draft_id=o.id),'slicer_jobs',(SELECT count(*) FROM slicer_jobs WHERE model_id='${model.id}')) FROM orders o JOIN manual_quotes m ON m.order_id=o.id WHERE o.id='${result.order_id}'`,
  );
  expect(persisted).toEqual({
    parts: expected.map((p) => ({
      ...p,
      requested_color: null,
      options: { ...print, quantity: p.quantity },
    })),
    amount: null,
    offer: null,
    payment_attempt: null,
    files: 2,
    payment_attempts: 0,
    slicer_jobs: 0,
  });
  const duplicate = await page.request.post('/api/account/manual-quotes', {
    headers: await headers(page),
    data: { ...body, request_id: randomUUID(), parts: [body.parts[0], body.parts[0]] },
  });
  expect(duplicate.status()).toBe(400);
  expect(await duplicate.json()).toMatchObject({ code: 'invalid_manual_parts' });
  expect(uploads).toBe(1);
  const adminContext = await browser.newContext();
  try {
    const operator = await adminContext.newPage(),
      errors: string[] = [];
    operator.on('pageerror', (error) => errors.push(error.name));
    const admin = accounts().find((a) => a.role === 'admin')!;
    await login(operator, admin);
    await operator.getByRole('button', { name: `Otwórz ${result.order_id}`, exact: true }).click();
    const detail = operator.getByRole('complementary', {
      name: `Szczegóły ${result.order_id}`,
      exact: true,
    });
    await expect(detail.getByLabel('Etap realizacji')).toBeDisabled();
    const adminRows = detail.locator('section.model');
    await expect(adminRows).toHaveCount(2);
    const files = (
      await (await operator.request.get(`/api/admin/orders/${result.order_id}/files`)).json()
    ).files;
    expect(new Set(files.map((f: { quote_id: string }) => f.quote_id))).toEqual(
      new Set(order.lines.map((l: { quote_id: string }) => l.quote_id)),
    );
    for (const [index, line] of order.lines.entries()) {
      const file = files.find((f: { quote_id: string }) => f.quote_id === line.quote_id);
      expect(file).toMatchObject({
        ...expected[index],
        options: { ...print, quantity: expected[index].quantity },
      });
      await expect(adminRows.nth(index).locator('.material-tag')).toHaveText(
        file.material_name || file.material_id,
      );
      const count = adminRows.nth(index).getByRole('spinbutton', { name: /Gotowe \/ / });
      await expect(count).toBeDisabled();
      await expect(count).toHaveValue('0');
      await expect(count).toHaveAttribute('max', String(expected[index].quantity));
    }
    expect(new Set(files.map((f: { source_url: string }) => f.source_url))).toEqual(
      new Set([`/api/admin/orders/${result.order_id}/files/${model.id}/source`]),
    );
    const path = `/api/admin/orders/${result.order_id}/production`;
    const before = await (await operator.request.get(path)).json();
    const rejected = await operator.request.put(path, {
      headers: await headers(operator),
      data: {
        ...before,
        progress: order.lines.map((l: { line_id: string }, i: number) => ({
          model_id: l.line_id,
          done: i === 0 ? 1 : 0,
        })),
      },
    });
    expect(rejected.status()).toBe(422);
    expect(await rejected.json()).toMatchObject({
      message: expect.stringContaining('Postęp wymaga opłaconego zamówienia'),
    });
    expect(await (await operator.request.get(path)).json()).toEqual(before);
    const unchanged = await (
      await page.request.get(`/api/account/orders/${result.order_id}`)
    ).json();
    expect(unchanged).toMatchObject({
      status: 'awaiting_quote',
      paid_at: null,
      amount_grosze: null,
      payment_attempt_id: null,
      manual_offer: { payment_started: false },
    });
    expect(unchanged.lines).toEqual(order.lines);
    expect(errors).toEqual([]);
    await test.info().attach('manual-admin-unpaid-evidence', {
      body: JSON.stringify({ files, production: before, rejected_status: rejected.status() }),
      contentType: 'application/json',
    });
  } finally {
    await adminContext.close();
  }

  await test.info().attach('manual-variants-evidence', {
    body: JSON.stringify({
      model_id: model.id,
      upload_count: uploads,
      order_id: result.order_id,
      lines: order.lines,
      persisted,
      exact_duplicate_status: duplicate.status(),
    }),
    contentType: 'application/json',
  });
});
