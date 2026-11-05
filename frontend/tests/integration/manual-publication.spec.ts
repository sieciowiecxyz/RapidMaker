import { expect, test, type Page } from './fixtures';
import { accounts, login } from './helpers';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Authentication is real; no credentials or session/payment secrets are captured.
test.use({ screenshot: 'off', trace: 'off' });

async function headers(page: Page) {
  const me = await (await page.request.get('/api/auth/me')).json();
  expect(me.stage).toBe('authenticated');
  return { Origin: 'http://localhost:18437', 'X-CSRF-Token': me.csrf_token };
}

function offerEmails(id: string) {
  const database = process.env.RAPIDMAKER_TEST_DATABASE ?? '';
  if (
    !/^rapidmaker_test_browser_[a-f0-9]+$/.test(database) ||
    !/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(id)
  )
    throw new Error('Offer inspection requires an isolated database and owned order UUID');
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
        `SELECT COALESCE(json_agg(json_build_object('event_key',event_key,'status',status,'attempts',attempts) ORDER BY event_key),'[]'::json) FROM email_outbox WHERE kind='manual_offer' AND payload->'notification'->>'reference'='${id}'`,
      ],
      { encoding: 'utf8' },
    ).trim(),
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
