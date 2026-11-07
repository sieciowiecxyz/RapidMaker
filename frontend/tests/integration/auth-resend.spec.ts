import { expect, test } from './fixtures';
import { accounts, mail } from './helpers';

test.use({ screenshot: 'off', trace: 'off', video: 'off' });

test('real email MFA resend invalidates the original code and consumed pending session', async ({
  page,
  context,
  browser,
}) => {
  test.skip(process.env.RAPIDMAKER_SANDBOX !== '1', 'Requires real sandbox SMTP/IMAP');
  test.setTimeout(240_000);
  const client = accounts().find((account) => account.email === 'void@sieciowiec.xyz');
  if (!client || client.totp_uri) throw new Error('Email MFA client fixture required');
  const origin = 'http://localhost:18437';
  const initialCursor = await mail({ mode: 'watermark' });
  await page.goto('/logowanie', { waitUntil: 'domcontentloaded' });
  await page.getByLabel('Adres email', { exact: true }).fill(client.email);
  await page.getByLabel('Hasło', { exact: true }).fill(client.password);
  const primaryResponse = page.waitForResponse((response) =>
    response.url().endsWith('/api/auth/password/login'),
  );
  await page.getByRole('button', { name: 'Zaloguj hasłem', exact: true }).click();
  expect((await primaryResponse).status()).toBe(200);
  let lastSentAt = Date.now();
  const field = page.getByLabel('Kod z emaila', { exact: true });
  await expect(field).toBeVisible();
  const pending = await (await page.request.get('/api/auth/me')).json();
  expect(pending.stage).toBe('second_factor');
  expect(pending.second_factor).toBe('email');
  expect((await page.request.get('/api/account/orders')).status()).toBe(401);
  const original = (await mail({ mode: 'code', after: initialCursor.after })).code;
  let replacement = original;
  // Wait actual elapsed time; never backdate DB state. One extra resend handles
  // the rare valid random-code collision without printing either code.
  for (let attempt = 0; attempt < 2 && replacement === original; attempt++) {
    const remaining = Math.max(0, 61_000 - (Date.now() - lastSentAt));
    if (remaining > 0) await page.waitForTimeout(remaining);
    const freshCursor = await mail({ mode: 'watermark' });
    const resent = page.waitForResponse((response) =>
      response.url().endsWith('/api/auth/factor/resend'),
    );
    await page.getByRole('button', { name: 'Wyślij kod ponownie', exact: true }).click();
    expect((await resent).status()).toBe(200);
    lastSentAt = Date.now();
    await expect(page.getByRole('status')).toHaveText('Wysłano nowy kod.');
    replacement = (await mail({ mode: 'code', after: freshCursor.after })).code;
  }
  if (replacement === original)
    throw new Error('Repeated random code collision prevents invalidation evidence');

  const rejected = page.waitForResponse((response) =>
    response.url().endsWith('/api/auth/factor/verify'),
  );
  await field.fill(original);
  await page.getByRole('button', { name: 'Potwierdź i zaloguj', exact: true }).click();
  expect((await rejected).status()).toBe(401);
  await expect(page.getByRole('alert')).toHaveText('Kod jest nieprawidłowy, zużyty lub wygasł.');
  expect((await (await page.request.get('/api/auth/me')).json()).stage).toBe('second_factor');
  expect((await page.request.get('/api/account/orders')).status()).toBe(401);
  const pendingCookies = await context.cookies();
  const accepted = page.waitForResponse((response) =>
    response.url().endsWith('/api/auth/factor/verify'),
  );
  await field.fill(replacement);
  await page.getByRole('button', { name: 'Potwierdź i zaloguj', exact: true }).click();
  expect((await accepted).status()).toBe(200);
  await expect
    .poll(async () => (await (await page.request.get('/api/auth/me')).json()).stage)
    .toBe('authenticated');
  expect((await page.request.get('/api/account/orders')).status()).toBe(200);

  const replay = await browser.newContext();
  try {
    await replay.addCookies(pendingCookies);
    expect(
      (
        await replay.request.post(`${origin}/api/auth/factor/verify`, {
          headers: { Origin: origin, 'X-CSRF-Token': pending.csrf_token },
          data: { code: replacement },
        })
      ).status(),
    ).toBe(401);
    expect((await replay.request.get(`${origin}/api/auth/me`)).status()).toBe(401);
    expect((await replay.request.get(`${origin}/api/account/orders`)).status()).toBe(401);
  } finally {
    await replay.close();
  }
});
