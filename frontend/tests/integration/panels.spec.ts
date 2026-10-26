import { expect, test, type Page } from './fixtures';
import { randomUUID } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { accounts, login, fillStripeCard } from './helpers';

// Auth, addresses and Stripe secrets must not be recorded in traces or screenshots.
test.use({ screenshot: 'off', trace: 'off' });

async function headers(page: Page) {
  const me = await (await page.request.get('/api/auth/me')).json();
  expect(me.stage).toBe('authenticated');
  return { Origin: 'http://localhost:18437', 'X-CSRF-Token': me.csrf_token };
}

function addOwnHistoricalMessages(orderId: string, userId: string) {
  const database = process.env.RAPIDMAKER_TEST_DATABASE;
  const uuid = /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
  if (
    !database ||
    !/^rapidmaker_test_browser_[a-f0-9]+$/.test(database) ||
    !uuid.test(orderId) ||
    !uuid.test(userId)
  )
    throw new Error('Historical fixture requires an isolated database and owned order UUID');
  // Historical chat is fixture data only; no paid status, provider result or notification is fabricated.
  const count = execFileSync(
    'podman',
    [
      'exec',
      process.env.TEST_PG_CONTAINER || 'rapidmaker-catalog-postgres',
      'psql',
      '-U',
      'rapidmaker',
      '-d',
      database,
      '-v',
      'ON_ERROR_STOP=1',
      '-At',
      '-c',
      `WITH inserted AS (INSERT INTO order_messages(id,order_id,author_id,operator,body,created_at) SELECT gen_random_uuid()::text,o.id,o.user_id,false,'Historia paginacji E2E '||n,extract(epoch from now())::bigint-60 FROM orders o CROSS JOIN generate_series(1,52) n WHERE o.id='${orderId}' AND o.user_id='${userId}' RETURNING id) SELECT count(*) FROM inserted`,
    ],
    { encoding: 'utf8' },
  ).trim();
  expect(count).toBe('52');
}

async function workspaceAdminViews(operator: Page, clientPage: Page, id: string) {
  const number = `RM-${id.slice(0, 8).toUpperCase()}`;
  await operator.goto('/admin/zamowienia');
  const search = operator.getByLabel('Szukaj', { exact: true });
  const status = operator.getByRole('combobox', { name: 'Status', exact: true });
  await search.fill(number);
  for (const value of [
    'awaiting_payment',
    'review',
    'quote_expired',
    'preparing',
    'printing',
    'shipped',
    'ready',
    'completed',
    'cancelled',
  ]) {
    await status.selectOption(value);
    if (value === 'preparing')
      await expect(
        operator
          .getByRole('table', { name: 'Zamówienia', exact: true })
          .getByRole('row')
          .filter({ hasText: number }),
      ).toHaveCount(1);
    else await expect(operator.getByRole('heading', { name: 'Brak zamówień' })).toBeVisible();
  }
  await status.selectOption('');
  await operator.getByRole('checkbox', { name: 'Wymaga reakcji', exact: true }).check();
  await operator.getByRole('checkbox', { name: 'Wymaga reakcji', exact: true }).uncheck();
  await search.fill('nieistniejace-' + randomUUID());
  await expect(operator.getByRole('heading', { name: 'Brak zamówień' })).toBeVisible();
  // This view resets through its fields; it has no independent reset button.
  await search.clear();
  await search.fill(number);
  await operator
    .getByRole('table', { name: 'Zamówienia', exact: true })
    .getByRole('row')
    .filter({ hasText: number })
    .getByRole('button', { name: 'Otwórz ↗', exact: true })
    .click();
  await expect(operator).toHaveURL(new RegExp(`/admin/zamowienia/${id}$`));
  for (const name of [
    'Modele i ustawienia',
    'Płatność i dostawa',
    'Produkcja',
    'Płatność i kontakt',
    'Rozmowa o zamówieniu',
  ])
    await expect(operator.getByRole('heading', { name, exact: true })).toBeVisible();
  await operator.getByText('Parametry wydruku', { exact: true }).click();
  await expect(operator.getByLabel('Podgląd modelu cube_20mm.stl')).toBeVisible();
  await expect(operator.getByLabel('Termin realizacji', { exact: true })).toBeDisabled();
  await expect(operator.getByLabel('Prywatne notatki zespołu', { exact: true })).toBeDisabled();
  const download = operator.waitForEvent('download');
  await operator.getByRole('link', { name: 'Pobierz oryginał ↗', exact: true }).click();
  expect((await download).suggestedFilename()).toBe('cube_20mm.stl');
  const marker = 'Rozmowa workspace ' + randomUUID();
  await operator.getByLabel('Wiadomość', { exact: true }).fill(marker);
  await operator.getByRole('button', { name: 'Wyślij wiadomość', exact: true }).click();
  await expect(operator.getByText(marker, { exact: true })).toBeVisible();
  await clientPage.reload();
  await expect(clientPage.getByText(marker, { exact: true })).toBeVisible();
  await operator.getByRole('link', { name: '← Wszystkie zamówienia', exact: true }).click();
  await expect(operator).toHaveURL(/\/admin\/zamowienia$/);
  await operator.goto('/admin/produkcja');
  const job = operator.locator('.production-job').filter({ hasText: number });
  await expect(job).toBeVisible();
  await expect(job.getByText('Przygotowanie', { exact: true })).toBeVisible();
  await expect(job.getByRole('button', { name: 'Rozpocznij druk', exact: true })).toBeEnabled();
  await job.getByRole('link', { name: 'Szczegóły →', exact: true }).click();
  await expect(operator).toHaveURL(new RegExp(`/admin/zamowienia/${id}$`));
}

test('real client/admin panels, files, chat, production, partial/full Stripe refunds and permissions', async ({
  page,
  browser,
}, info) => {
  test.skip(
    process.env.RAPIDMAKER_SANDBOX !== '1',
    'Requires actual Stripe test and isolated account fixtures',
  );
  test.setTimeout(300_000);
  const fixture = accounts();
  const client = fixture.find((a) => a.email === 'void@sieciowiec.xyz')!;
  const admin = fixture.find((a) => a.role === 'admin')!;
  const other = fixture.find((a) => a.email === 'browser-other@example.invalid')!;
  const adminContext = await browser.newContext(),
    otherContext = await browser.newContext();
  try {
    const operator = await adminContext.newPage(),
      outsider = await otherContext.newPage();
    await login(page, client);
    await login(operator, admin);
    await login(outsider, other);

    // A genuine upload and durable manual request avoid slicing the same geometry repeatedly.
    await page.goto('/wycena', { waitUntil: 'domcontentloaded' });
    const uploaded = page.waitForResponse(
      (r) => r.url().endsWith('/api/models') && r.request().method() === 'POST',
    );
    await page
      .getByLabel('Dodaj modele STL, STEP lub 3MF')
      .setInputFiles(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
    const upload = await uploaded;
    expect(upload.status()).toBe(201);
    // WebGL work can evict inspector response bodies. Read the actual server ID persisted by UI.
    const uploadedId = () =>
      page.evaluate(() => {
        const draft = JSON.parse(localStorage.getItem('rapidmaker.storefront.draft.v1') ?? 'null');
        return draft?.models?.find((m: { name: string }) => m.name === 'cube_20mm.stl')?.analysis
          ?.id;
      });
    await expect.poll(uploadedId).toBeTruthy();
    const metadata = await page.request.get(`/api/models/${await uploadedId()}`);
    expect(metadata.status()).toBe(200);
    const model = await metadata.json();
    expect(model.filename).toBe('cube_20mm.stl');
    const catalog = await (await page.request.get('/api/catalog/materials')).json();
    const color = catalog.materials.find((m: { id: string }) => m.id === 'GFA00').colors[0].code;
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
          name: 'Panel integration',
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
        notes: 'Automatyczny test paneli. Odbiór osobisty.',
      },
    });
    expect(created.status()).toBe(200);
    const { order_id: id } = await created.json();
    await operator.goto(`/admin/zamowienia/${id}`, { waitUntil: 'domcontentloaded' });
    await operator.getByLabel('Całość brutto (zł)').fill('25,00');
    await operator.getByLabel('W tym dostawa (zł)').fill('0,00');
    await operator
      .getByLabel('Zakres wykonania i dostawy')
      .fill('Wydruk testowy, odbiór osobisty, do 14 dni.');
    operator.once('dialog', (dialog) => dialog.accept());
    await operator.getByRole('button', { name: 'Opublikuj ofertę i powiadom klienta' }).click();
    await expect
      .poll(
        async () =>
          (await (await page.request.get(`/api/account/orders/${id}`)).json()).manual_offer.state,
      )
      .toBe('offered');
    await page.goto(`/konto/zamowienia/${id}`, { waitUntil: 'domcontentloaded' });
    await page.getByRole('button', { name: 'Przygotuj płatność za ofertę' }).click();
    await fillStripeCard(page);
    await page.getByRole('button', { name: /Zamawiam i płacę 25/ }).click();
    await expect
      .poll(
        async () => (await (await page.request.get(`/api/account/orders/${id}`)).json()).status,
        { timeout: 40_000 },
      )
      .toBe('paid');

    await workspaceAdminViews(operator, page, id);

    // Client list, detail, persisted preferences and two-way browser chat.
    await page.goto('/konto/zamowienia');
    const row = page.locator('.order-row').filter({ has: page.locator(`strong[title="${id}"]`) });
    await expect(row).toBeVisible();
    await row.getByRole('link', { name: /Szczegóły zamówienia/ }).click();
    await page.getByText('Parametry wydruku', { exact: true }).click();
    await expect(page.getByText('Ściany: 3; wypełnienie: 20%')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Zamów ponownie ↗' })).toBeDisabled();
    await expect(page.getByLabel('Podgląd modelu cube_20mm.stl')).toBeVisible();
    const original = page.waitForEvent('download');
    await page.getByRole('link', { name: 'Pobierz oryginał ↗' }).click();
    expect((await original).suggestedFilename()).toBe('cube_20mm.stl');
    const clientMessage = `Klient UI ${randomUUID()}`;
    await page.getByLabel('Wiadomość', { exact: true }).fill(clientMessage);
    await page.getByRole('button', { name: 'Wyślij wiadomość', exact: true }).click();
    await expect(page.getByText(clientMessage, { exact: true })).toBeVisible();
    await page.goto('/konto/ustawienia');
    const statusPreference = page.getByRole('checkbox', {
      name: 'Zmiany statusu, wysyłka i gotowość do odbioru',
    });
    const chatPreference = page.getByRole('checkbox', { name: 'Nowe wiadomości od obsługi' });
    await statusPreference.uncheck();
    await chatPreference.uncheck();
    await page.getByRole('button', { name: 'Zapisz ustawienia', exact: true }).click();
    await expect(page.getByText('Zapisano preferencje powiadomień na koncie.')).toBeVisible();
    await page.reload();
    await expect(statusPreference).not.toBeChecked();
    await expect(chatPreference).not.toBeChecked();
    await statusPreference.check();
    await chatPreference.check();
    await page.getByRole('button', { name: 'Zapisz ustawienia', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Usunięcie konta' })).toBeVisible();
    await expect(page.getByRole('link', { name: 'piotr@rapidmaker.pl' })).toHaveAttribute(
      'href',
      /mailto:piotr@rapidmaker\.pl/,
    );

    addOwnHistoricalMessages(id, client.id);

    // Admin list tools, CSV with full UUID, theme and every drawer tab.
    await operator.goto('/admin.html');
    const nav = operator.getByRole('navigation', { name: 'Panel administratora' });
    await operator.getByRole('button', { name: 'Włącz jasny motyw' }).click();
    await expect(operator.locator('html')).toHaveAttribute('data-theme', 'light');
    await operator.getByRole('button', { name: 'Włącz ciemny motyw' }).click();
    for (const filter of ['Wymaga reakcji', 'Nowe', 'W produkcji', 'Do wysłania', 'Wszystkie']) {
      const control = operator
        .getByLabel('Filtr zamówień')
        .getByRole('button', { name: filter, exact: filter !== 'Wszystkie' });
      await control.click();
      await expect(control).toHaveAttribute('aria-pressed', 'true');
    }
    await operator.getByLabel('Szukaj zamówień').fill('does-not-exist-' + randomUUID());
    await expect(operator.getByRole('heading', { name: 'Brak pasujących zamówień' })).toBeVisible();
    await operator.getByRole('button', { name: 'Wyczyść filtry', exact: true }).click();
    await operator.getByLabel('Szukaj zamówień').fill(id);
    const csv = operator.waitForEvent('download');
    await operator.getByRole('button', { name: 'Eksport CSV', exact: true }).click();
    const saved = await csv;
    expect(saved.suggestedFilename()).toBe('rapidmaker-zamowienia.csv');
    expect(readFileSync((await saved.path())!, 'utf8')).toContain(id);
    await operator.getByRole('button', { name: 'Wyczyść wyszukiwanie' }).click();
    await operator.getByRole('button', { name: `Otwórz ${id}`, exact: true }).click();
    const detail = operator.getByRole('complementary', { name: `Szczegóły ${id}` });
    await expect(detail).toBeVisible();
    await expect(detail.getByLabel('Termin realizacji')).toBeDisabled();
    await expect(detail.getByLabel('Cena / szt. (zł)')).toBeDisabled();
    await detail.getByRole('button', { name: 'Pliki', exact: true }).click();
    await expect(detail.getByLabel('Podgląd modelu cube_20mm.stl')).toBeVisible();
    await expect(detail.getByRole('button', { name: 'Zmień plik' })).toBeDisabled();
    const adminDownload = operator.waitForEvent('download');
    await detail.getByRole('link', { name: /Pobierz/ }).click();
    expect((await adminDownload).suggestedFilename()).toBe('cube_20mm.stl');
    await detail.getByRole('button', { name: 'Wiadomości', exact: true }).click();
    await expect(detail.getByText(clientMessage, { exact: true })).toBeVisible();
    await expect(detail.getByText('Historia paginacji E2E 52', { exact: true })).toHaveCount(0);
    await expect(detail.getByRole('button', { name: 'Pokaż kolejne', exact: true })).toBeVisible();
    await detail.getByRole('button', { name: 'Pokaż kolejne', exact: true }).click();
    await expect(detail.getByText('Historia paginacji E2E 52', { exact: true })).toBeVisible();
    await expect(detail.getByRole('button', { name: 'Pokaż kolejne', exact: true })).toHaveCount(0);
    const history = (
      await (
        await operator.request.get(`/api/admin/orders/${id}/messages?after=0&limit=100`)
      ).json()
    ).messages;
    await expect(detail.locator('.bubble')).toHaveCount(history.length);

    const reply = `Operator UI ${randomUUID()}`;
    await detail.getByLabel('Twoja odpowiedź').fill(reply);
    await detail.getByRole('button', { name: 'Wyślij wiadomość', exact: true }).click();
    await expect(detail.getByText(reply, { exact: true })).toBeVisible();
    await page.goto(`/konto/zamowienia/${id}#rozmowa`);
    await expect(page.getByText(reply, { exact: true })).toBeVisible();
    const unread = page.getByRole('button', { name: /^Oznacz \d+ jako przeczytane$/ });
    if (await unread.count()) {
      await unread.click();
      await expect(unread).toHaveCount(0);
    }
    await detail.getByRole('button', { name: 'Dostawa', exact: true }).click();
    await expect(detail.getByText('Odbiór osobisty — bez nadawania przesyłki.')).toBeVisible();
    await detail
      .getByRole('region', { name: 'Dostawa zamówienia' })
      .getByRole('button', { name: 'Odśwież' })
      .click();
    await detail.getByRole('button', { name: 'Historia', exact: true }).click();
    await expect(detail.getByRole('heading', { name: 'Historia realizacji' })).toBeVisible();
    await operator.keyboard.press('Escape');
    await expect(detail).toHaveCount(0);

    // Two independent controls advance only valid server statuses; pickup can complete without a carrier.
    await operator.getByRole('button', { name: `Otwórz ${id}`, exact: true }).click();
    await detail.getByLabel('Etap realizacji').selectOption('in_production');
    await expect
      .poll(async () => (await (await page.request.get(`/api/account/orders/${id}`)).json()).status)
      .toBe('in_production');
    await operator.goto('/admin/produkcja');
    const workspaceJob = operator
      .locator('.production-job')
      .filter({ hasText: `RM-${id.slice(0, 8).toUpperCase()}` });
    await expect(workspaceJob).toBeVisible();
    await expect(workspaceJob.getByText('W druku', { exact: true })).toBeVisible();
    await expect(
      workspaceJob.getByRole('button', { name: 'Gotowe do odbioru', exact: true }),
    ).toBeEnabled();
    await workspaceJob.getByRole('link', { name: 'Szczegóły →', exact: true }).click();
    await expect(operator.getByLabel('Status', { exact: true })).toHaveValue('printing');
    await expect(
      operator.getByRole('button', { name: 'Zapisz status', exact: true }),
    ).toBeDisabled();
    await operator.goto('/admin.html');
    await operator.getByRole('button', { name: `Otwórz ${id}`, exact: true }).click();
    await detail.getByRole('button', { name: /^Dalej:/ }).click();
    await expect
      .poll(async () => (await (await page.request.get(`/api/account/orders/${id}`)).json()).status)
      .toBe('ready');
    await detail.getByRole('button', { name: 'Zamknij szczegóły' }).click();
    await nav.getByRole('button', { name: 'Produkcja', exact: true }).click();
    const readyCard = operator.getByRole('article', { name: `Zlecenie produkcyjne ${id}` });
    await expect(readyCard).toBeVisible();
    await readyCard.getByRole('button', { name: /Dalej/ }).click();
    await expect
      .poll(async () => (await (await page.request.get(`/api/account/orders/${id}`)).json()).status)
      .toBe('completed');

    await nav.getByRole('button', { name: 'Płatności', exact: true }).click();
    for (const filter of ['Nieopłacone', 'Opłacone', 'Wszystkie']) {
      await operator
        .getByLabel('Filtr płatności')
        .getByRole('button', { name: filter, exact: true })
        .click();
    }
    await operator.getByLabel('Szukaj płatności').fill('does-not-exist-' + randomUUID());
    await operator.getByRole('button', { name: 'Wyczyść filtry' }).click();
    await operator.getByLabel('Szukaj płatności').fill(id);
    await operator.getByRole('button', { name: 'Szczegóły rozliczenia' }).click();
    const refunds = detail.getByRole('region', { name: 'Zwroty płatności' });
    await expect(refunds.getByLabel('Kwota zwrotu (zł)')).toBeEnabled();
    await expect(detail.getByRole('button', { name: 'Wystaw fakturę' })).toBeDisabled();
    await refunds.getByLabel('Kwota zwrotu (zł)').fill('25,01');
    await refunds.getByRole('button', { name: 'Zleć zwrot', exact: true }).click();
    await expect(refunds.getByRole('alert')).toContainText('nie może przekraczać');
    expect(await (await operator.request.get(`/api/admin/orders/${id}/refunds`)).json()).toEqual(
      [],
    );
    await refunds.getByLabel('Kwota zwrotu (zł)').fill('5,00');
    operator.once('dialog', (dialog) => dialog.accept());
    await refunds.getByRole('button', { name: 'Zleć zwrot', exact: true }).click();
    await expect
      .poll(
        async () => {
          const result = await (
            await operator.request.get(`/api/admin/orders/${id}/refunds`)
          ).json();
          return (
            result.length === 1 &&
            result[0].state === 'succeeded' &&
            result[0].amount_grosze === 500
          );
        },
        { timeout: 70_000, intervals: [1000, 3000] },
      )
      .toBe(true);
    const firstRefund = (
      await (await operator.request.get(`/api/admin/orders/${id}/refunds`)).json()
    )[0];
    const repeated = await operator.request.post(`/api/admin/orders/${id}/refunds`, {
      headers: await headers(operator),
      data: { request_id: firstRefund.request_id, amount_grosze: 500 },
    });
    expect(repeated.status()).toBe(202);
    expect((await repeated.json()).id).toBe(firstRefund.id);
    expect(
      (await (await operator.request.get(`/api/admin/orders/${id}/refunds`)).json()).length,
    ).toBe(1);
    await refunds.getByRole('button', { name: 'Odśwież', exact: true }).click();
    await expect(refunds.getByLabel('Kwota zwrotu (zł)')).toBeEnabled();
    await refunds.getByRole('button', { name: 'Cała pozostała kwota' }).click();
    await expect(refunds.getByLabel('Kwota zwrotu (zł)')).toHaveValue('20,00');
    operator.once('dialog', (dialog) => dialog.accept());
    await refunds.getByRole('button', { name: 'Zleć zwrot', exact: true }).click();
    await expect
      .poll(
        async () => {
          const result: { state: string; amount_grosze: number }[] = await (
            await operator.request.get(`/api/admin/orders/${id}/refunds`)
          ).json();
          return (
            result.length === 2 &&
            result.every((r) => r.state === 'succeeded') &&
            result.reduce((sum, r) => sum + r.amount_grosze, 0) === 2500
          );
        },
        { timeout: 70_000, intervals: [1000, 3000] },
      )
      .toBe(true);
    await refunds.getByRole('button', { name: 'Odśwież', exact: true }).click();
    await expect(refunds.getByLabel('Kwota zwrotu (zł)')).toHaveCount(0);
    expect((await (await page.request.get(`/api/account/orders/${id}`)).json()).status).toBe(
      'completed',
    );

    const forbidden = await page.request.post(`/api/admin/orders/${id}/refunds`, {
      headers: await headers(page),
      data: { request_id: randomUUID(), amount_grosze: 1 },
    });
    expect(forbidden.status()).toBe(403);
    expect((await outsider.request.get(`/api/account/orders/${id}`)).status()).toBe(404);
    expect((await outsider.request.get(`/api/account/orders/${id}/files`)).status()).toBe(404);
    expect((await outsider.request.get(`/api/account/orders/${id}/messages`)).status()).toBe(404);
    const overRefund = await operator.request.post(`/api/admin/orders/${id}/refunds`, {
      headers: await headers(operator),
      data: { request_id: randomUUID(), amount_grosze: 1 },
    });
    expect(overRefund.status()).toBe(409);
    await detail.getByRole('button', { name: 'Zamknij szczegóły' }).click();
    await nav.getByRole('button', { name: 'Faktury', exact: true }).click();
    await expect(operator.getByRole('button', { name: 'Wystaw fakturę' })).toBeDisabled();
    await nav.getByRole('button', { name: 'Wiadomości', exact: true }).click();
    await operator.getByRole('button', { name: new RegExp(`zamówienie ${id}$`) }).click();
    await detail.getByRole('button', { name: 'Pokaż kolejne', exact: true }).click();
    await expect(detail.getByText(reply, { exact: true })).toBeVisible();
    await detail.getByRole('button', { name: 'Zamknij szczegóły' }).click();
    await operator.getByRole('button', { name: 'Odśwież', exact: true }).click();
    await operator.screenshot({ path: info.outputPath('admin-desktop.png') });
    await operator.setViewportSize({ width: 390, height: 844 });
    expect(await operator.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
      true,
    );
    await operator.screenshot({ path: info.outputPath('admin-mobile.png') });
    await operator.setViewportSize({ width: 1440, height: 1000 });
    await operator.getByRole('button', { name: 'Wyloguj', exact: true }).click();
    await expect(operator).toHaveURL(/\/logowanie/);
    expect((await operator.request.get('/api/admin/orders')).status()).toBe(401);
    await page.goto('/konto/zamowienia');
    await page.setViewportSize({ width: 390, height: 844 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
      true,
    );
    await page.screenshot({ path: info.outputPath('client-mobile.png') });
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.getByRole('button', { name: 'Wyloguj', exact: true }).click();
    await expect(page).toHaveURL('http://localhost:18437/');
    expect((await page.request.get('/api/account/orders')).status()).toBe(401);
  } finally {
    await adminContext.close();
    await otherContext.close();
  }
});

test('real operator catalog/color edits and all pricing fields persist, invalid rates cannot save', async ({
  page,
}) => {
  test.skip(
    process.env.RAPIDMAKER_SANDBOX !== '1',
    'Requires isolated authenticated operator fixture',
  );
  const admin = accounts().find((a) => a.role === 'admin')!;
  await login(page, admin);
  await page.goto('/admin/materialy');
  await page.getByLabel('Znajdź materiał').fill('nonexistent-material');
  await expect(page.locator('.materials-list > details')).toHaveCount(0);
  await page.getByLabel('Znajdź materiał').fill('');
  const add = page.locator('details.add-material');
  await add.locator('summary').click();
  const materialId = 'E2E_' + randomUUID().replaceAll('-', '').slice(0, 12);
  const materialName = 'Bambu integration ' + materialId;
  await add.getByLabel('Identyfikator', { exact: true }).fill(materialId);
  await add.getByLabel('Nazwa', { exact: true }).fill(materialName);
  await add.getByLabel('Gęstość (g/cm³)', { exact: true }).fill('1,24');
  await add.getByLabel('Cena zakupu netto (zł/kg)', { exact: true }).fill('99,99');
  await add.getByRole('button', { name: 'Dodaj materiał', exact: true }).click();
  await expect(add.getByRole('status')).toHaveText('Zapisano.');
  await page.getByLabel('Znajdź materiał').fill(materialId);
  let editor = page.locator('.materials-list > details').filter({ hasText: materialId });
  await editor.locator(':scope > summary').click();
  const materialForm = editor.locator('.editor > form').first();
  await materialForm.getByLabel('Nazwa', { exact: true }).fill(materialName + ' edited');
  await materialForm.getByLabel('Gęstość (g/cm³)', { exact: true }).fill('1,25');
  await materialForm.getByLabel('Cena zakupu netto (zł/kg)', { exact: true }).fill('100,01');
  await editor.getByRole('checkbox', { name: /Dopuszczam materiał/ }).check();
  await editor.getByRole('checkbox', { name: /Dopuszczam materiał/ }).uncheck();
  const saved = page.waitForResponse(
    (r) => r.url().endsWith(`/api/admin/materials/${materialId}`) && r.request().method() === 'PUT',
  );
  await editor.getByRole('button', { name: 'Zapisz materiał', exact: true }).click();
  expect((await saved).status()).toBe(200);
  const newColor = editor.locator('details.add-color');
  await newColor.locator('summary').click();
  await newColor.getByLabel('Kod', { exact: true }).fill('E2E_COLOR');
  await newColor.getByLabel('Nazwa', { exact: true }).fill('Testowa zieleń');
  await newColor.getByLabel('Kolor RGBA').fill('#22AA66FF');
  await newColor.getByRole('button', { name: 'Dodaj kolor', exact: true }).click();
  const colorRow = editor.locator('form.color-row');
  await expect(colorRow).toHaveCount(1);
  await colorRow.getByLabel(/Nazwa/).fill('Zieleń po zmianie');
  await colorRow.getByText('Kolor', { exact: true }).click();
  await colorRow.getByLabel('RGBA 1', { exact: true }).fill('#119944FF');
  const colorSaved = page.waitForResponse(
    (r) =>
      r.url().endsWith(`/api/admin/materials/${materialId}/colors/E2E_COLOR`) &&
      r.request().method() === 'PUT',
  );
  await colorRow.getByRole('button', { name: 'Zapisz kolor', exact: true }).click();
  expect((await colorSaved).status()).toBe(200);
  await page.reload();
  await page.getByLabel('Znajdź materiał').fill(materialId);
  editor = page.locator('.materials-list > details').filter({ hasText: materialId });
  await editor.locator(':scope > summary').click();
  await expect(
    editor.locator('.editor > form').first().getByLabel('Nazwa', { exact: true }),
  ).toHaveValue(materialName + ' edited');
  await expect(
    editor
      .locator('.editor > form')
      .first()
      .getByLabel('Cena zakupu netto (zł/kg)', { exact: true }),
  ).toHaveValue('100.01');
  await expect(editor.locator('form.color-row').getByLabel(/Nazwa/)).toHaveValue(
    'Zieleń po zmianie',
  );

  await page.goto('/admin/ustawienia');
  const version = 'browser-panel-' + randomUUID();
  await page.locator('#rates-version').fill(version);
  const fields: Record<string, string> = {
    setup_grosze: '10,00',
    handling_per_plate_grosze: '5,00',
    machine_per_hour_grosze: '2,00',
    energy_per_kwh_grosze: '1,20',
    printer_watts: '150',
    loss_bps: '5,00',
    margin_bps: '100,00',
    minimum_net_grosze: '20,00',
    vat_bps: '23,00',
  };
  for (const [field, value] of Object.entries(fields)) await page.locator('#' + field).fill(value);
  await page.getByRole('button', { name: 'Zapisz cennik', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Marża musi wynosić');
  await page.locator('#margin_bps').fill('30,00');
  const ratesSaved = page.waitForResponse(
    (r) => r.url().endsWith('/api/admin/pricing') && r.request().method() === 'PUT',
  );
  await page.getByRole('button', { name: 'Zapisz cennik', exact: true }).click();
  expect((await ratesSaved).status()).toBe(200);
  await page.reload();
  await expect(page.locator('#rates-version')).toHaveValue(version);
  await expect(page.locator('#margin_bps')).toHaveValue('30.00');
  const rates = (await (await page.request.get('/api/admin/pricing')).json()).rates;
  expect(rates).toMatchObject({
    version,
    setup_grosze: 1000,
    handling_per_plate_grosze: 500,
    machine_per_hour_grosze: 200,
    energy_per_kwh_grosze: 120,
    printer_watts: 150,
    loss_bps: 500,
    margin_bps: 3000,
    minimum_net_grosze: 2000,
    vat_bps: 2300,
  });
});
