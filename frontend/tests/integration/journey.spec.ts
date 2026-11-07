import { expect, test } from './fixtures';
import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';
import { randomUUID } from 'node:crypto';
import { accounts, login, mail, fillStripeCard } from './helpers';
import { fillAuthorContact } from './checkout-helpers';

// Only inspect the disposable runner database, with validated SQL values and no mail tokens.
function inboundReplay(order: string, uidvalidity: number, uids: number[]) {
  const database = process.env.RAPIDMAKER_TEST_DATABASE ?? '';
  if (
    !/^rapidmaker_test_browser_[a-f0-9]{32}$/.test(database) ||
    !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(order) ||
    !Number.isSafeInteger(uidvalidity) ||
    uidvalidity <= 0 ||
    uids.length !== 2 ||
    new Set(uids).size !== 2 ||
    uids.some((uid) => !Number.isSafeInteger(uid) || uid <= 0)
  )
    throw new Error('Inbound inspection requires the isolated database and two IMAP deliveries');
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
        `SELECT json_build_object(
      'outcomes', COALESCE(json_agg(outcome ORDER BY uid), '[]'::json),
      'same_digest', count(DISTINCT encode(digest,'hex'))=1,
      'same_mailbox', count(DISTINCT mailbox)=1,
      'order_matches', COALESCE(bool_or(message_id IN (SELECT id FROM order_messages WHERE order_id='${order}' AND origin='email')),false)
    ) FROM chat_inbound WHERE uidvalidity=${uidvalidity} AND uid IN (${uids.join(',')})`,
      ],
      { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], timeout: 15_000 },
    ).trim(),
  );
}

test.use({ screenshot: 'off', trace: 'off' });
test('real manual quote, admin publication, iCloud notice, Stripe payment/refund and email reply', async ({
  page,
  browser,
}) => {
  test.skip(
    process.env.RAPIDMAKER_SANDBOX !== '1',
    'Requires --sandbox; sends only to void and charges only Stripe test',
  );
  test.setTimeout(480_000);
  const fixture = accounts();
  await login(
    page,
    fixture.find((a) => a.email === 'void@sieciowiec.xyz')!,
  );
  await page.goto('/wycena', { waitUntil: 'domcontentloaded' });
  await page
    .getByLabel('Dodaj modele STL, STEP lub 3MF')
    .setInputFiles(resolve('../backend/crates/geometry/tests/fixtures/huge_cube_1000mm.stl'));
  await expect(page.getByRole('img', { name: /Model z pliku huge_cube_1000mm.stl/ })).toBeVisible();
  await page.locator('input[name=material][value=pla]').check();
  await page.locator('input[name=color]').first().check();
  await page.locator('input[name=quality][value=standard]').check();
  await page.locator('input[name=strength][value=standard]').check();
  await expect(page.getByText('Wymagana ręczna wycena', { exact: true })).toBeVisible();
  await page.locator('.quote-contact').click();
  await fillAuthorContact(page);
  await page.getByRole('button', { name: 'Przejdź do indywidualnej wyceny', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Zapytaj o wycenę', exact: true })).toBeVisible();
  await page.getByLabel('Imię i nazwisko', { exact: true }).fill('Test Rapidmaker');
  await page.getByLabel('Telefon', { exact: true }).fill('500600700');
  await page.getByRole('radio', { name: 'Odbiór osobisty', exact: true }).check();
  await page.getByRole('checkbox', { name: 'Chcę fakturę na firmę', exact: true }).check();
  await page.getByLabel('Nazwa firmy', { exact: true }).fill('Firma testowa');
  await page.getByLabel('NIP', { exact: true }).fill('1234567883');
  await page.getByRole('checkbox', { name: 'Chcę fakturę na firmę', exact: true }).uncheck();
  await page.getByLabel('Uwagi i wymagania').fill('Podział modelu na części, odbiór osobisty.');
  await page.getByRole('button', { name: 'Wyślij zapytanie o wycenę', exact: true }).click();
  await expect(page).toHaveURL(/\/konto\/zamowienia\/[a-f0-9-]{36}/);
  const id = new URL(page.url()).pathname.split('/').at(-1)!;
  const initial = await (await page.request.get(`/api/account/orders/${id}`)).json();
  expect(initial.contact.invoice ?? null).toBeNull();
  expect(initial.amount_grosze).toBeNull();
  expect(initial.manual_offer.state).toBe('awaiting_quote');
  const adminContext = await browser.newContext(),
    otherContext = await browser.newContext();
  try {
    const adminPage = await adminContext.newPage(),
      otherPage = await otherContext.newPage();
    await login(
      adminPage,
      fixture.find((a) => a.role === 'admin')!,
    );
    await login(
      otherPage,
      fixture.find((a) => a.email === 'browser-other@example.invalid')!,
    );
    expect((await otherPage.request.get(`/api/account/orders/${id}`)).status()).toBe(404);
    expect((await otherPage.request.get(`/api/account/orders/${id}/files`)).status()).toBe(404);
    await adminPage.goto(`/admin/zamowienia/${id}`, { waitUntil: 'domcontentloaded' });
    await adminPage.getByLabel('Całość brutto (zł)').fill('25,00');
    await adminPage.getByLabel('W tym dostawa (zł)').fill('0,00');
    await adminPage
      .getByLabel('Zakres wykonania i dostawy')
      .fill('Podział modelu na części, odbiór osobisty.');
    const cursor = await mail({ mode: 'watermark' });
    adminPage.once('dialog', (dialog) => dialog.accept());
    await adminPage.getByRole('button', { name: 'Opublikuj ofertę i powiadom klienta' }).click();
    await expect
      .poll(
        async () =>
          (await (await page.request.get(`/api/account/orders/${id}`)).json()).manual_offer.state,
      )
      .toBe('offered');
    // Keep checking payment even if provider mail delivery fails; the test still fails.
    let offerReceived = false;
    try {
      offerReceived = (await mail({ mode: 'offer', reference: id, after: cursor.after })).received;
    } catch {
      /* Report below without leaking mail/credentials. */
    }
    expect.soft(offerReceived, 'Published offer must actually arrive in iCloud').toBe(true);
    await page.reload({ waitUntil: 'domcontentloaded' });
    const files = await (await page.request.get(`/api/account/orders/${id}/files`)).json();
    expect(files.files.length).toBe(1);
    await expect(page.getByLabel(`Podgląd modelu ${files.files[0].filename}`)).toBeVisible();
    const me = await (await page.request.get('/api/auth/me')).json();
    const denied = await page.request.post(`/api/admin/orders/${id}/quote`, {
      headers: { Origin: 'http://localhost:18437', 'X-CSRF-Token': me.csrf_token },
      data: { version: 2, amount_grosze: 1, shipping_grosze: 0, scope: 'forbidden' },
    });
    expect(denied.status()).toBe(403);
    await page.getByRole('button', { name: 'Przygotuj płatność za ofertę' }).click();
    await fillStripeCard(page);
    const refreshed = page.waitForResponse(
      (r) =>
        r.url().endsWith(`/api/account/orders/${id}/payment-status`) &&
        r.request().method() === 'POST',
    );
    await page.getByRole('button', { name: /Zamawiam i płacę 25/ }).click();
    expect((await refreshed).status()).toBe(200);
    await expect
      .poll(
        async () => (await (await page.request.get(`/api/account/orders/${id}`)).json()).status,
        { timeout: 40_000 },
      )
      .toBe('paid');
    expect((await mail({ mode: 'legal', reference: id, after: cursor.after })).received).toBe(true);
    console.log(
      'Manual offer: UI request/admin publication/private preview/Stripe paid and terms email PASS',
    );
    const adminMe = await (await adminPage.request.get('/api/auth/me')).json();
    const headers = { Origin: 'http://localhost:18437', 'X-CSRF-Token': adminMe.csrf_token };
    const refundCursor = await mail({ mode: 'watermark' });
    const refundData = { request_id: randomUUID(), amount_grosze: 500 };
    const refundUrl = `/api/admin/orders/${id}/refunds`;
    const refundResponse = await adminPage.request.post(refundUrl, { headers, data: refundData });
    expect(refundResponse.status()).toBe(202);
    const refund = await refundResponse.json();
    const retry = await adminPage.request.post(refundUrl, { headers, data: refundData });
    expect(retry.status()).toBe(202);
    expect((await retry.json()).id).toBe(refund.id);
    await expect
      .poll(
        async () => {
          const order = await (await page.request.get(`/api/account/orders/${id}`)).json();
          const refunds = await (await adminPage.request.get(refundUrl)).json();
          return {
            amount: order.refunded_grosze,
            state: refunds.find((entry: { id: string }) => entry.id === refund.id)?.state,
          };
        },
        { timeout: 90_000, intervals: [1000, 3000] },
      )
      .toEqual({ amount: 500, state: 'succeeded' });
    expect(
      (await mail({ mode: 'refund', reference: id, amount_grosze: 500, after: refundCursor.after }))
        .received,
    ).toBe(true);
    const mailCursor = await mail({ mode: 'watermark' });
    const marker = 'Browser chat ' + randomUUID();
    const sent = await adminPage.request.post(`/api/admin/orders/${id}/messages`, {
      headers,
      data: { body: marker },
    });
    expect(sent.status()).toBe(200);
    const copy: any = await mail({ mode: 'chat', reference: id, marker, after: mailCursor.after });
    const reply = 'Email reply ' + randomUUID();
    const replyCursor = await mail({ mode: 'watermark' });
    const outboundMessageId = `<${randomUUID()}@sieciowiec.xyz>`;
    const replay = {
      mode: 'reply',
      message_id: copy.message_id,
      body: reply,
      outbound_message_id: outboundMessageId,
    };
    await mail(replay);
    await mail(replay);
    // SMTP acceptance alone is insufficient: require two actual provider deliveries.
    const deliveries = await mail({
      mode: 'deliveries',
      after: replyCursor.after,
      outbound_message_id: outboundMessageId,
      marker: reply,
    });
    const { uidvalidity, uids } = deliveries;
    if (uidvalidity === undefined || !uids) throw new Error('Missing IMAP delivery evidence');
    expect(uids).toHaveLength(2);
    await expect
      .poll(() => inboundReplay(id, uidvalidity, uids), {
        timeout: 90_000,
        intervals: [1000, 3000],
      })
      .toEqual({
        outcomes: ['imported', 'duplicate'],
        same_digest: true,
        same_mailbox: true,
        order_matches: true,
      });
    await expect
      .poll(
        async () => {
          const history = await (
            await page.request.get(`/api/account/orders/${id}/messages`)
          ).json();
          return history.messages.filter((m: { body: string }) => m.body.includes(reply)).length;
        },
        { timeout: 90_000, intervals: [1000, 3000] },
      )
      .toBe(1);
    await page.reload({ waitUntil: 'domcontentloaded' });
    await expect(page.getByText(reply, { exact: true })).toBeVisible();
    console.log(
      'Email replay: two real IMAP UIDs, durable imported/duplicate, one client chat message PASS',
    );
  } finally {
    await adminContext.close();
    await otherContext.close();
  }
});
