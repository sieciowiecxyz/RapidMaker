import { expect, test } from '@playwright/test';
import { accounts, login, mail } from './helpers';

test.use({ screenshot: 'off', trace: 'off' });
test('real password/email, native passkey/email and admin password/TOTP', async ({
  page,
  context,
  browser,
}) => {
  test.skip(
    process.env.RAPIDMAKER_SANDBOX !== '1',
    'Requires --sandbox and the private iCloud fixture',
  );
  test.setTimeout(240_000);
  const fixture = accounts(),
    client = fixture.find((a) => a.email === 'void@sieciowiec.xyz')!;
  await login(page, client);
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
  await page.goto('/konto/ustawienia', { waitUntil: 'domcontentloaded' });
  await page.getByLabel('Nazwa nowego klucza').fill('Browser integration');
  const registered = page.waitForResponse((r) =>
    r.url().endsWith('/api/auth/passkey/register/finish'),
  );
  await page.getByRole('button', { name: 'Dodaj klucz dostępu', exact: true }).click();
  expect((await registered).status()).toBe(200);
  const me = await (await page.request.get('/api/auth/me')).json();
  expect(me.stage).toBe('authenticated');
  const logout = await page.request.post('/api/auth/logout', {
    headers: { Origin: 'http://localhost:18437', 'X-CSRF-Token': me.csrf_token },
    data: {},
  });
  expect(logout.status()).toBe(200);
  const cursor = await mail({ mode: 'watermark' });
  await page.goto('/logowanie', { waitUntil: 'domcontentloaded' });
  await page.getByLabel('Adres email', { exact: true }).fill(client.email);
  await page.getByRole('button', { name: 'Zaloguj passkey', exact: true }).click();
  await expect(page.getByLabel('Kod z emaila')).toBeVisible();
  expect((await (await page.request.get('/api/auth/me')).json()).stage).toBe('second_factor');
  const code = await mail({ mode: 'code', after: cursor.after });
  await page.getByLabel('Kod z emaila').fill(code.code);
  await page.getByLabel('Kod z emaila').press('Enter');
  await expect
    .poll(async () => (await (await page.request.get('/api/auth/me')).json()).stage)
    .toBe('authenticated');
  const adminContext = await browser.newContext();
  try {
    const adminPage = await adminContext.newPage();
    await login(
      adminPage,
      fixture.find((a) => a.role === 'admin')!,
    );
    const admin = await (await adminPage.request.get('/api/auth/me')).json();
    expect(admin.admin_verified).toBe(true);
    expect((await adminPage.request.get('/api/admin/orders')).status()).toBe(200);
    expect((await page.request.get('/api/admin/orders')).status()).toBe(403);
  } finally {
    await adminContext.close();
  }
});
