import { expect, type Page, type Frame } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { spawn } from 'node:child_process';
import { createHmac } from 'node:crypto';
import { resolve } from 'node:path';
export type Account = {
  id: string;
  email: string;
  role: string;
  password: string;
  totp_uri: string | null;
};
export function accounts(): Account[] {
  const path = process.env.RAPIDMAKER_TEST_ACCOUNTS;
  if (!path?.startsWith('/tmp/rapidmaker-browser-')) throw new Error('Isolated fixture missing');
  return JSON.parse(readFileSync(path, 'utf8')).accounts;
}
export function mail(
  input: Record<string, unknown>,
): Promise<{ after: number; code: string; received: boolean }> {
  return new Promise((resolveResult, reject) => {
    const child = spawn('python', [resolve('tests/read-test-mail.py')], {
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    let output = '',
      failure = '';
    child.stderr.on('data', (data) => {
      if (failure.length < 512) failure += data;
    });
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error('Fresh test mail timed out'));
    }, 100_000);
    child.stdout.on('data', (data) => {
      output += data;
      if (output.length > 4096) child.kill();
    });
    child.on('error', () => {
      clearTimeout(timer);
      reject(new Error('Mail reader unavailable'));
    });
    child.on('close', (code) => {
      clearTimeout(timer);
      if (code !== 0)
        return reject(
          new Error(
            failure.startsWith('Test mail unavailable (')
              ? failure.trim()
              : 'Fresh test mail unavailable; no mail body logged',
          ),
        );
      try {
        resolveResult(JSON.parse(output));
      } catch {
        reject(new Error('Invalid private mail result'));
      }
    });
    child.stdin.end(JSON.stringify(input));
  });
}
export function totp(uri: string, timestamp = Date.now()): string {
  const secret = new URL(uri).searchParams.get('secret')!;
  let bits = '';
  for (const char of secret.replace(/=+$/, ''))
    bits += 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'.indexOf(char).toString(2).padStart(5, '0');
  const bytes = Buffer.from(
    Array.from({ length: Math.floor(bits.length / 8) }, (_, i) =>
      parseInt(bits.slice(i * 8, i * 8 + 8), 2),
    ),
  );
  const step = Buffer.alloc(8);
  step.writeBigUInt64BE(BigInt(Math.floor(timestamp / 30_000)));
  const hash = createHmac('sha1', bytes).update(step).digest(),
    offset = hash[hash.length - 1] & 15;
  return ((hash.readUInt32BE(offset) & 0x7fffffff) % 1_000_000).toString().padStart(6, '0');
}
export async function login(page: Page, account: Account) {
  const cursor = account.totp_uri ? null : await mail({ mode: 'watermark' });
  await page.goto('/logowanie', { waitUntil: 'domcontentloaded' });
  await page.getByLabel('Adres email', { exact: true }).fill(account.email);
  await page.getByLabel('Hasło', { exact: true }).fill(account.password);
  await page.getByRole('button', { name: 'Zaloguj hasłem', exact: true }).click();
  const code = page.getByLabel(account.totp_uri ? 'Kod z aplikacji' : 'Kod z emaila', {
    exact: true,
  });
  await expect(code).toBeVisible();
  const pending = await (await page.request.get('/api/auth/me')).json();
  expect(pending.stage).toBe('second_factor');
  if (account.totp_uri)
    await expect(page.getByRole('button', { name: /Wyślij.*ponownie/ })).toHaveCount(0);
  const value = account.totp_uri
    ? totp(account.totp_uri)
    : (await mail({ mode: 'code', after: cursor!.after })).code;
  await code.fill(value);
  const verified = page.waitForResponse((r) => r.url().endsWith('/api/auth/factor/verify'));
  await code.press('Enter');
  const response = await verified;
  if (response.status() === 401 && account.totp_uri) {
    // A previous test may have consumed this 30-second step; never bypass replay protection.
    await new Promise((resolveWait) => setTimeout(resolveWait, 30_200 - (Date.now() % 30_000)));
    await code.fill(totp(account.totp_uri));
    const retry = page.waitForResponse((r) => r.url().endsWith('/api/auth/factor/verify'));
    await code.press('Enter');
    expect((await retry).status()).toBe(200);
  } else expect(response.status()).toBe(200);
  await expect
    .poll(async () => (await (await page.request.get('/api/auth/me')).json()).stage)
    .toBe('authenticated');
  await expect(page).toHaveURL(
    account.role === 'admin' ? /\/admin\.html$/ : /\/konto\/zamowienia$/,
  );
}

export async function fillStripeCard(page: Page, number = '4242424242424242') {
  let payment: Frame | undefined;
  await expect
    .poll(
      async () => {
        for (const frame of page.frames()) {
          if (!frame.url().startsWith('https://js.stripe.com/')) continue;
          const card = frame.getByRole('button', { name: 'Karta', exact: true });
          if (await card.isVisible().catch(() => false)) await card.click();
          if (
            frame.url().startsWith('https://js.stripe.com/') &&
            (await frame
              .locator('input[name="number"]')
              .isVisible()
              .catch(() => false))
          ) {
            payment = frame;
            return true;
          }
        }
        return false;
      },
      { timeout: 30_000, message: 'Stripe card fields must load in their own iframe' },
    )
    .toBe(true);
  if (!payment) throw new Error('Stripe card frame missing');
  await payment.locator('input[name="number"]').fill(number);
  await payment.locator('input[name="expiry"]').fill('1234');
  await payment.locator('input[name="cvc"]').fill('123');
  const country = payment.locator('select[name="country"]');
  if (await country.count()) await country.selectOption('PL');
  const postcode = payment.locator('input[name="postalCode"]');
  if (await postcode.count()) await postcode.fill('05-500');
}
