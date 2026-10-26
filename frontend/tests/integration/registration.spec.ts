import { test, expect } from './fixtures';
import { mail } from './helpers';

test.use({ screenshot: 'off', trace: 'off' });
const method = process.env.E2E_REGISTRATION_METHOD ?? 'password';
test(`real voluntary ${method} registration verifies email and preserves the private account`, async ({
  page,
  context,
}) => {
  test.skip(process.env.RAPIDMAKER_SANDBOX !== '1', 'Requires --sandbox --registration');
  test.setTimeout(180_000);
  const cursor = await mail({ mode: 'watermark' });
  await page.goto('/logowanie?register=1');
  await expect(page.getByText('Tworząc konto, akceptujesz', { exact: false })).toBeVisible();
  await page.getByRole('button', { name: 'Mam już konto', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Zaloguj hasłem', exact: true })).toBeVisible();
  await page
    .getByRole('button', { name: 'Utwórz konto lub zabezpiecz dotychczasowe', exact: true })
    .click();
  await page.getByLabel('Adres email', { exact: true }).fill('void@sieciowiec.xyz');
  await page.getByRole('button', { name: 'Utwórz konto — potwierdź email', exact: true }).click();
  await expect(page.getByLabel('Kod z emaila', { exact: true })).toBeVisible();
  // Immediate resend must be bounded and must not invalidate the first still-valid code.
  await page.getByRole('button', { name: 'Wyślij kod ponownie', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  const delivered = await mail({ mode: 'code', after: cursor.after });
  await page.getByLabel('Kod z emaila', { exact: true }).fill(delivered.code);
  await page.getByRole('button', { name: 'Potwierdź i zaloguj', exact: true }).click();
  await expect(page.getByLabel('Nowe hasło', { exact: true })).toBeVisible();
  expect((await (await page.request.get('/api/auth/me')).json()).stage).toBe('setup');
  expect((await page.request.get('/api/account/orders')).status()).toBe(401);
  const password = 'Rapidmaker-registration-test-2026!';
  if (method === 'passkey') {
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
    await page.getByLabel('Nazwa nowego klucza').fill('First registration key');
    await page.getByRole('button', { name: 'Dodaj klucz dostępu', exact: true }).click();
  } else {
    await page.getByLabel('Nowe hasło', { exact: true }).fill(password);
    await page.getByLabel('Powtórz hasło', { exact: true }).fill(password);
    await page.getByRole('button', { name: 'Ustaw hasło i przejdź dalej', exact: true }).click();
  }
  await expect(page).toHaveURL(/\/konto\/zamowienia$/);
  const session = await (await page.request.get('/api/auth/me')).json();
  expect(session.stage).toBe('authenticated');
  expect(session.user.email).toBe('void@sieciowiec.xyz');
  expect(session.user.role).toBe('client');
  expect(
    (await mail({ mode: 'legal', reference: session.user.id, after: cursor.after })).received,
  ).toBe(true);
  expect((await page.request.get('/api/admin/orders')).status()).toBe(403);
  await page.goto('/konto/ustawienia');
  if (method === 'passkey') {
    await expect(page.getByRole('button', { name: 'Usuń klucz', exact: true })).toBeDisabled();
    const keys = await (await page.request.get('/api/auth/passkeys')).json();
    const forbidden = await page.request.delete(`/api/auth/passkeys/${keys.passkeys[0].id}`, {
      headers: { Origin: new URL(page.url()).origin, 'X-CSRF-Token': session.csrf_token },
    });
    expect(forbidden.status()).toBe(403);
    await page.getByLabel('Nowe hasło', { exact: true }).fill(password);
    await page.getByLabel('Powtórz hasło', { exact: true }).fill(password);
    await page.getByRole('button', { name: 'Zapisz hasło', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Usuń klucz', exact: true })).toBeEnabled();
  } else {
    await expect(page.getByRole('heading', { name: 'Zmień hasło', exact: true })).toBeVisible();
  }
  await expect(page.getByRole('heading', { name: 'Usunięcie konta', exact: true })).toBeVisible();
});
