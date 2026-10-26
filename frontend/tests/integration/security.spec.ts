import { expect, test } from './fixtures';
import { accounts, login, totp } from './helpers';

test.use({ screenshot: 'off', trace: 'off' });

test('real login rejects wrong password, TOTP and recovery without granting access', async ({
  page,
}) => {
  test.skip(!process.env.RAPIDMAKER_TEST_ACCOUNTS, 'Requires isolated accounts');
  const account = accounts().find((a) => a.email === 'browser-other@example.invalid')!;
  await page.goto('/logowanie?next=https%3A%2F%2Fexample.invalid');
  await page.getByLabel('Adres email', { exact: true }).fill(account.email);
  await page.getByLabel('Hasło', { exact: true }).fill('Wrong-password-123!');
  await page.getByRole('button', { name: 'Zaloguj hasłem', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  expect((await page.request.get('/api/account/orders')).status()).toBe(401);
  await page.getByLabel('Hasło', { exact: true }).fill(account.password);
  await page.getByRole('button', { name: 'Zaloguj hasłem', exact: true }).click();
  const valid = new Set(
    [-30_000, 0, 30_000].map((delta) => totp(account.totp_uri!, Date.now() + delta)),
  );
  let bad = '000000';
  while (valid.has(bad)) bad = (Number(bad) + 1).toString().padStart(6, '0');
  await page.getByLabel('Kod z aplikacji', { exact: true }).fill(bad);
  await page.getByRole('button', { name: 'Potwierdź i zaloguj', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  expect((await page.request.get('/api/account/orders')).status()).toBe(401);
  await page.getByRole('button', { name: 'Użyj kodu ratunkowego', exact: true }).click();
  await page.getByLabel('Kod ratunkowy', { exact: true }).fill('invalid-recovery-code');
  await page.getByRole('button', { name: 'Potwierdź i zaloguj', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await page.getByRole('button', { name: 'Wróć do kodu z aplikacji', exact: true }).click();
  await page.getByLabel('Kod z aplikacji', { exact: true }).fill(totp(account.totp_uri!));
  await page.getByRole('button', { name: 'Potwierdź i zaloguj', exact: true }).click();
  await expect(page).toHaveURL(/\/konto\/zamowienia$/);
  expect((await page.request.get('/api/admin/orders')).status()).toBe(403);
});

test('real security settings rotate sessions, change password, use passkey and replace TOTP', async ({
  page,
  context,
  browser,
}) => {
  test.skip(!process.env.RAPIDMAKER_TEST_ACCOUNTS, 'Requires --local-accounts or sandbox accounts');
  test.setTimeout(180_000);
  const account = accounts().find((a) => a.email === 'browser-security@example.invalid')!;
  await page.goto('/konto/ustawienia');
  await expect(page.getByRole('button', { name: 'Zaloguj hasłem', exact: true })).toBeVisible();
  await login(page, account);
  await page.goto('/konto/ustawienia');
  const security = page.locator('section[aria-labelledby="account-security-heading"]');
  const keys = page.locator('section[aria-labelledby="passkeys-heading"]');
  await security.getByRole('button', { name: 'Odśwież stan', exact: true }).click();
  await expect(security.getByRole('heading', { name: 'Zmień hasło', exact: true })).toBeVisible();
  const stale = await browser.newContext({ storageState: await context.storageState() });
  const password = account.password + '-changed';
  await security.getByLabel('Nowe hasło', { exact: true }).fill('short');
  await security.getByLabel('Powtórz hasło', { exact: true }).fill('short');
  await security.getByRole('button', { name: 'Zapisz hasło', exact: true }).click();
  await expect(security.getByRole('alert')).toContainText('12');
  await security.getByLabel('Nowe hasło', { exact: true }).fill(password);
  await security.getByLabel('Powtórz hasło', { exact: true }).fill(password + 'different');
  await security.getByRole('button', { name: 'Zapisz hasło', exact: true }).click();
  await expect(security.getByRole('alert')).toContainText(/hasła są różne/);
  await security.getByLabel('Powtórz hasło', { exact: true }).fill(password);
  const saved = page.waitForResponse(
    (r) => r.url().endsWith('/api/account/password') && r.request().method() === 'PUT',
  );
  await security.getByRole('button', { name: 'Zapisz hasło', exact: true }).click();
  expect((await saved).status()).toBe(200);
  expect((await stale.request.get('/api/auth/me')).status()).toBe(401);
  await stale.close();
  const probe = await browser.newContext();
  try {
    const endpoint = new URL('/api/auth/password/login', page.url()).href;
    const headers = { Origin: new URL(page.url()).origin };
    expect(
      (
        await probe.request.post(endpoint, {
          headers,
          data: { email: account.email, password: account.password },
        })
      ).status(),
    ).toBe(401);
    const primary = await probe.request.post(endpoint, {
      headers,
      data: { email: account.email, password },
    });
    expect(primary.status()).toBe(200);
    expect((await primary.json()).stage).toBe('second_factor');
    expect(
      (await probe.request.get(new URL('/api/account/orders', page.url()).href)).status(),
    ).toBe(401);
  } finally {
    await probe.close();
  }

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
  await keys.getByLabel('Nazwa nowego klucza').fill('E2E security key');
  const registered = page.waitForResponse((r) =>
    r.url().endsWith('/api/auth/passkey/register/finish'),
  );
  await keys.getByRole('button', { name: 'Dodaj klucz dostępu', exact: true }).click();
  expect((await registered).status()).toBe(200);
  await expect(keys.getByText('E2E security key', { exact: true })).toBeVisible();

  page.on('dialog', (dialog) => dialog.accept());
  await security.getByRole('button', { name: 'Wyloguj wszystkie urządzenia', exact: true }).click();
  await expect(page).toHaveURL(/\/logowanie$/);
  await page.getByLabel('Adres email', { exact: true }).fill(account.email);
  await page.getByRole('button', { name: 'Zaloguj passkey', exact: true }).click();
  await expect(page.getByLabel('Kod z aplikacji', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Użyj kodu ratunkowego', exact: true }).click();
  await expect(page.getByLabel('Kod ratunkowy', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Wróć do kodu z aplikacji', exact: true }).click();
  // Respect replay protection across the password and passkey logins.
  await new Promise((resolve) => setTimeout(resolve, 30_200 - (Date.now() % 30_000)));
  await page.getByLabel('Kod z aplikacji', { exact: true }).fill(totp(account.totp_uri!));
  await page.getByRole('button', { name: 'Potwierdź i zaloguj', exact: true }).click();
  await expect
    .poll(async () => (await (await page.request.get('/api/auth/me')).json()).stage)
    .toBe('authenticated');
  await page.goto('/konto/ustawienia');
  await security.getByRole('button', { name: 'Wyłącz aplikację', exact: true }).click();
  await expect(
    security.getByRole('button', { name: 'Włącz aplikację', exact: true }),
  ).toBeVisible();
  const setupResponse = page.waitForResponse((r) => r.url().endsWith('/api/account/totp/start'));
  await security.getByRole('button', { name: 'Włącz aplikację', exact: true }).click();
  const setup = await (await setupResponse).json();
  await security.getByLabel('Kod z aplikacji', { exact: true }).fill(totp(setup.otpauth_uri));
  await security.getByRole('button', { name: 'Potwierdź włączenie', exact: true }).click();
  await expect(security.locator('.recovery-codes code')).toHaveCount(8);
  const recovery = await security.locator('.recovery-codes code').first().innerText();
  await security.getByRole('button', { name: 'Zapisałem kody — ukryj', exact: true }).click();
  await expect(security.locator('.recovery-codes code')).toHaveCount(0);
  await keys.getByRole('button', { name: 'Usuń klucz', exact: true }).click();
  await expect(keys.getByText('E2E security key', { exact: true })).toHaveCount(0);
  await security.getByLabel('Nowe hasło', { exact: true }).fill(account.password);
  await security.getByLabel('Powtórz hasło', { exact: true }).fill(account.password);
  await security.getByRole('button', { name: 'Zapisz hasło', exact: true }).click();
  await expect(security.getByRole('status')).toContainText('Hasło zapisane');
  await security.getByRole('button', { name: 'Wyloguj wszystkie urządzenia', exact: true }).click();
  await expect(page).toHaveURL(/\/logowanie$/);
  await page.getByLabel('Adres email', { exact: true }).fill(account.email);
  await page.getByLabel('Hasło', { exact: true }).fill(account.password);
  await page.getByRole('button', { name: 'Zaloguj hasłem', exact: true }).click();
  await page.getByRole('button', { name: 'Użyj kodu ratunkowego', exact: true }).click();
  await page.getByLabel('Kod ratunkowy', { exact: true }).fill(recovery);
  await page.getByRole('button', { name: 'Potwierdź i zaloguj', exact: true }).click();
  await expect(page).toHaveURL(/\/konto\/zamowienia$/);
  expect((await page.request.get('/api/admin/orders')).status()).toBe(403);
  await page.goto('/konto/ustawienia');
  await security.getByRole('button', { name: 'Wyloguj wszystkie urządzenia', exact: true }).click();
  await expect(page).toHaveURL(/\/logowanie$/);
  await page.getByLabel('Adres email', { exact: true }).fill(account.email);
  await page.getByLabel('Hasło', { exact: true }).fill(account.password);
  await page.getByRole('button', { name: 'Zaloguj hasłem', exact: true }).click();
  await page.getByRole('button', { name: 'Użyj kodu ratunkowego', exact: true }).click();
  await page.getByLabel('Kod ratunkowy', { exact: true }).fill(recovery);
  await page.getByRole('button', { name: 'Potwierdź i zaloguj', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  expect((await page.request.get('/api/account/orders')).status()).toBe(401);
  await page.getByRole('button', { name: 'Zmień konto', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Zaloguj hasłem', exact: true })).toBeVisible();
});
