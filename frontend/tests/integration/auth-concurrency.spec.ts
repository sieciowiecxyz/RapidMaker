import { expect, test, type Page } from './fixtures';
import { accounts, totp, type Account } from './helpers';

test.use({ screenshot: 'off', trace: 'off', video: 'off' });

async function password(page: Page, account: Account) {
  await page.getByLabel('Adres email', { exact: true }).fill(account.email);
  await page.getByLabel('Hasło', { exact: true }).fill(account.password);
  await page.getByRole('button', { name: 'Zaloguj hasłem', exact: true }).click();
}
async function factor(page: Page, account: Account) {
  const field = page.getByLabel('Kod z aplikacji', { exact: true });
  await expect(field).toBeVisible();
  await field.fill(totp(account.totp_uri!));
  await field.press('Enter');
}

test('a delayed MFA response cannot overwrite a newer login from another tab', async ({
  page,
  context,
  playwright,
}) => {
  test.skip(
    !process.env.RAPIDMAKER_TEST_ACCOUNTS || process.env.RAPIDMAKER_SANDBOX === '1',
    'Requires disposable --local-accounts with providers and mail disabled',
  );
  const a = accounts().find((account) => account.email === 'browser-other@example.invalid')!;
  const b = accounts().find((account) => account.email === 'browser-security@example.invalid')!;
  expect(Boolean(a.totp_uri && b.totp_uri)).toBe(true);
  const second = await context.newPage();
  const proxy = await playwright.request.newContext({ baseURL: 'http://127.0.0.1:18436' });
  await page.goto('/logowanie');
  const primary = page.waitForResponse((r) => r.url().endsWith('/api/auth/password/login'));
  await password(page, a);
  expect((await primary).status()).toBe(200);

  let captured!: () => void, release!: () => void, delivered!: () => void;
  const held = new Promise<void>((resolve) => {
    captured = resolve;
  });
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const done = new Promise<void>((resolve) => {
    delivered = resolve;
  });
  await page.route('**/api/auth/factor/verify', async (route) => {
    // A separate cookie jar preserves the real Set-Cookie without applying it
    // to the browser before release. route.fetch would apply it prematurely.
    const request = route.request();
    const response = await proxy.fetch(new URL(request.url()).pathname, {
      method: request.method(),
      headers: { ...(await request.allHeaders()), host: new URL(request.url()).host },
      data: request.postDataBuffer()!,
    });
    expect(response.status()).toBe(200);
    expect((await response.json()).stage).toBe('authenticated');
    expect(!!response.headers()['set-cookie']).toBe(true);
    captured();
    await gate;
    await route.fulfill({ response });
    delivered();
  });
  try {
    const lateResponse = page.waitForResponse((r) => r.url().endsWith('/api/auth/factor/verify'));
    await factor(page, a);
    await held;
    // A's old pending cookie is now invalid, but the authenticated replacement is held.
    expect((await page.request.get('/api/auth/me')).status()).toBe(401);
    await second.goto('/logowanie?reauth=1');
    let started!: () => void;
    let secondRequests = 0;
    const secondStarted = new Promise<boolean>((resolve) => {
      started = () => resolve(true);
    });
    second.on('request', (request) => {
      if (request.url().endsWith('/api/auth/password/login')) {
        secondRequests++;
        started();
      }
    });
    await password(second, b);
    const overlapped = await Promise.race([
      secondStarted,
      new Promise<boolean>((resolve) => setTimeout(() => resolve(false), 500)),
    ]);
    let bAuthenticatedBeforeRelease = false;
    if (overlapped) {
      const verified = second.waitForResponse((r) => r.url().endsWith('/api/auth/factor/verify'));
      await factor(second, b);
      expect((await verified).status()).toBe(200);
      const me = await (await second.request.get('/api/auth/me')).json();
      bAuthenticatedBeforeRelease = me.stage === 'authenticated' && me.user.id === b.id;
      expect(bAuthenticatedBeforeRelease).toBe(true);
    }
    release();
    await done;
    expect((await lateResponse).status()).toBe(200);
    if (!overlapped) {
      // Session refresh may remount the form. After queued work settles, it must
      // not have sent B's stale attempt; the following retry is explicit.
      await expect
        .poll(() =>
          second.evaluate(async () => {
            const state = await navigator.locks.query();
            return state.held.length + state.pending.length;
          }),
        )
        .toBe(0);
      expect(secondRequests).toBe(0);
      const primaryB = second.waitForResponse((r) => r.url().endsWith('/api/auth/password/login'));
      await password(second, b);
      expect((await primaryB).status()).toBe(200);
      const verified = second.waitForResponse((r) => r.url().endsWith('/api/auth/factor/verify'));
      await factor(second, b);
      expect((await verified).status()).toBe(200);
    }
    const me = await (await second.request.get('/api/auth/me')).json();
    const finalIsB = me.stage === 'authenticated' && me.user.id === b.id;
    await test.info().attach('cookie-ordering-evidence', {
      body: JSON.stringify({
        overlapped,
        bAuthenticatedBeforeRelease,
        finalIsB,
        finalStage: me.stage,
      }),
      contentType: 'application/json',
    });
    expect(finalIsB, 'A stale response must not replace the later authenticated account B').toBe(
      true,
    );
    expect(overlapped, 'Cookie-changing requests must not overlap across tabs').toBe(false);
  } finally {
    release();
    await page.unroute('**/api/auth/factor/verify');
    await second.close();
    await proxy.dispose();
  }
});

test('session changes fail safely when browser locks are unavailable', async ({ page }) => {
  await page.addInitScript(() => Object.defineProperty(navigator, 'locks', { value: undefined }));
  let mutations = 0;
  page.on('request', (request) => {
    if (request.url().endsWith('/api/auth/password/login')) mutations++;
  });
  await page.goto('/logowanie');
  await page.getByLabel('Adres email', { exact: true }).fill('unused@example.invalid');
  await page.getByLabel('Hasło', { exact: true }).fill('not-a-real-password');
  await page.getByRole('button', { name: 'Zaloguj hasłem', exact: true }).click();
  await expect(
    page.getByText('Logowanie wymaga aktualnej przeglądarki i HTTPS lub localhost.', {
      exact: true,
    }),
  ).toBeVisible();
  expect(mutations).toBe(0);
});
