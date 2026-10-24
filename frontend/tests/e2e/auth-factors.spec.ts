import { expect, test } from '@playwright/test';
import { generateKeyPairSync } from 'node:crypto';

test('client passkey requires TOTP and never offers email fallback', async ({ page, context }) => {
  const cdp = await context.newCDPSession(page);
  await cdp.send('WebAuthn.enable');
  const { authenticatorId } = await cdp.send('WebAuthn.addVirtualAuthenticator', {
    options: {
      protocol: 'ctap2',
      transport: 'internal',
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });
  const id = Buffer.from('test-credential-for-client').toString('base64');
  const { privateKey } = generateKeyPairSync('ec', { namedCurve: 'prime256v1' });
  await cdp.send('WebAuthn.addCredential', {
    authenticatorId,
    credential: {
      credentialId: id,
      isResidentCredential: true,
      rpId: 'localhost',
      privateKey: privateKey.export({ type: 'pkcs8', format: 'der' }).toString('base64'),
      signCount: 0,
      userHandle: Buffer.from('client-1').toString('base64'),
    },
  });
  let stage: string | null = null,
    invalid = true,
    orderReads = 0;
  const writes: any[] = [];
  const state = () => ({
    user: { id: 'client-1', email: 'client@example.invalid', role: 'client' },
    stage,
    second_factor: 'totp',
    admin_verified: false,
    csrf_token: 'csrf-factor',
  });
  await page.route('**/api/**', async (route) => {
    const req = route.request(),
      path = new URL(req.url()).pathname;
    if (req.method() !== 'GET')
      writes.push({ path, body: req.postDataJSON(), csrf: req.headers()['x-csrf-token'] });
    let data: unknown;
    if (path === '/api/auth/me') {
      if (!stage) return route.fulfill({ status: 401, json: { message: 'Zaloguj się.' } });
      data = state();
    } else if (path.endsWith('/passkey/login/start'))
      data = {
        challenge_id: 'challenge-1',
        options: {
          publicKey: {
            challenge: Buffer.alloc(32, 1).toString('base64url'),
            rpId: 'localhost',
            allowCredentials: [
              { type: 'public-key', id: Buffer.from(id, 'base64').toString('base64url') },
            ],
            userVerification: 'required',
            timeout: 60000,
          },
        },
      };
    else if (path.endsWith('/passkey/login/finish')) {
      stage = 'second_factor';
      data = state();
    } else if (path === '/api/auth/factor/verify') {
      if (invalid) {
        invalid = false;
        return route.fulfill({ status: 401, json: { message: 'Kod jest nieprawidłowy.' } });
      }
      stage = 'authenticated';
      data = state();
    } else if (path === '/api/auth/claim-guest') data = { resources_claimed: 0 };
    else if (path === '/api/account/orders') {
      orderReads++;
      data = { orders: [] };
    } else throw new Error('Unexpected API ' + path);
    return route.fulfill({ json: data });
  });
  await page.goto('http://localhost:18431/logowanie');
  await page.getByLabel('Adres email', { exact: true }).fill('client@example.invalid');
  await page.getByRole('button', { name: 'Zaloguj passkey', exact: true }).click();
  await expect(page.getByLabel('Kod z aplikacji', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Wyślij kod ponownie', exact: true })).toHaveCount(
    0,
  );
  expect(orderReads).toBe(0);
  await page.getByLabel('Kod z aplikacji', { exact: true }).fill('111111');
  await page.getByRole('button', { name: 'Potwierdź i zaloguj', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Kod jest nieprawidłowy');
  await expect(page.getByLabel('Kod z aplikacji', { exact: true })).toBeVisible();
  await page.getByLabel('Kod z aplikacji', { exact: true }).fill('222222');
  await page.getByRole('button', { name: 'Potwierdź i zaloguj', exact: true }).click();
  await expect(page).toHaveURL(/\/konto\/zamowienia$/);
  await expect(page.getByText('Twój pierwszy projekt czeka', { exact: true })).toBeVisible();
  expect(writes.some((w) => w.path === '/api/auth/start' || w.path.endsWith('/resend'))).toBe(
    false,
  );
  expect(
    writes.find((w) => w.path.endsWith('/passkey/login/finish')).body.credential.response.signature,
  ).toBeTruthy();
  expect(
    writes.filter((w) => w.path.endsWith('/factor/verify')).every((w) => w.csrf === 'csrf-factor'),
  ).toBe(true);
});

test('account changes password and confirms authenticator without storing secrets', async ({
  page,
}) => {
  let enabled = false;
  const writes: any[] = [];
  await page.route('**/api/**', async (route) => {
    const req = route.request(),
      path = new URL(req.url()).pathname;
    if (req.method() !== 'GET')
      writes.push({ path, body: req.postDataJSON(), csrf: req.headers()['x-csrf-token'] });
    let data: unknown;
    if (path === '/api/auth/me')
      data = {
        user: { id: 'client-1', email: 'client@example.invalid', role: 'client' },
        stage: 'authenticated',
        second_factor: enabled ? 'totp' : 'email',
        csrf_token: 'csrf-settings',
        admin_verified: false,
      };
    else if (path === '/api/account/security')
      data = { has_password: true, totp_enabled: enabled, passkeys: [] };
    else if (path === '/api/auth/passkeys') data = { passkeys: [] };
    else if (path === '/api/account/notifications')
      data = { email_order_updates: true, email_chat_messages: true };
    else if (path === '/api/account/password') data = { ok: true };
    else if (path.endsWith('/totp/start'))
      data = {
        secret: 'JBSWY3DPEHPK3PXP',
        otpauth_uri: 'otpauth://totp/Rapidmaker:test?secret=JBSWY3DPEHPK3PXP&issuer=Rapidmaker',
      };
    else if (path.endsWith('/totp/confirm')) {
      enabled = true;
      data = { recovery_codes: ['test-backup-once-1', 'test-backup-once-2'] };
    } else throw new Error('Unexpected API ' + path);
    return route.fulfill({ json: data });
  });
  await page.goto('/konto/ustawienia');
  await expect(page.getByRole('button', { name: 'Włącz aplikację', exact: true })).toBeVisible();
  expect(writes).toHaveLength(0);
  await page.getByLabel('Nowe hasło', { exact: true }).fill('example secure password');
  await page.getByLabel('Powtórz hasło', { exact: true }).fill('different secure password');
  await page.getByRole('button', { name: 'Zapisz hasło', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Wpisane hasła są różne');
  expect(writes).toHaveLength(0);
  await page.getByLabel('Powtórz hasło', { exact: true }).fill('example secure password');
  await page.getByRole('button', { name: 'Zapisz hasło', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Hasło zapisane');
  await page.getByRole('button', { name: 'Włącz aplikację', exact: true }).click();
  await expect(page.getByText('JBSWY3DPEHPK3PXP', { exact: true })).toBeVisible();
  await page.getByLabel('Kod z aplikacji', { exact: true }).fill('123456');
  await page.getByRole('button', { name: 'Potwierdź włączenie', exact: true }).click();
  await expect(page.getByText('test-backup-once-1', { exact: true })).toBeVisible();
  expect(
    await page.evaluate(() => JSON.stringify({ ...sessionStorage, ...localStorage })),
  ).not.toMatch(/JBSWY3DPEHPK3PXP|test-backup-once|example secure password/);
  await page.getByRole('button', { name: 'Zapisałem kody — ukryj', exact: true }).click();
  await expect(page.getByText('test-backup-once-1', { exact: true })).toHaveCount(0);
  expect(writes.every((w) => w.csrf === 'csrf-settings')).toBe(true);
});
