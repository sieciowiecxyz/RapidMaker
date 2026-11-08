import { expect, test } from './fixtures';
import { accounts, login } from './helpers';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import ts from 'typescript';

test.use({ screenshot: 'off', trace: 'off', video: 'off' });

test('API JSON remains bound to its session when logout finishes between promise continuations', async ({ page }) => {
  test.skip(!process.env.RAPIDMAKER_TEST_ACCOUNTS, 'Requires --local-accounts');
  const account = accounts().find(account => account.email === 'browser-security@example.invalid');
  expect(account?.totp_uri).toBeTruthy();
  await login(page, account!);
  // Load the actual source modules without the view-level generation guards.
  // Transpilation preserves native async/await; only the import URL is changed.
  for (const [url, source] of [
    ['/owned-auth-sync.js', readFileSync(resolve('src/lib/authSync.ts'), 'utf8')],
    ['/owned-http.js', readFileSync(resolve('src/lib/http.ts'), 'utf8').replace("'./authSync'", "'/owned-auth-sync.js'")],
  ]) await page.route(`**${url}`, route => route.fulfill({ contentType: 'text/javascript', body: ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 },
  }).outputText }));
  await page.route('**/owned-api-probe', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><title>API session boundary</title>' }));
  await page.goto('/owned-api-probe');
  const result = await page.evaluate(async () => {
    const path = '/owned-http.js', syncPath = '/owned-auth-sync.js';
    const http = await import(path), sync = await import(syncPath);
    const initial = await http.apiRequest('/api/auth/me');
    if (!initial.user || initial.stage !== 'authenticated') throw new Error('Expected isolated authenticated fixture');
    const nativeFetch = window.fetch.bind(window);
    let armed = true, logoutStatus = 0, schedulerError = '';
    window.fetch = (...args) => {
      const pending = nativeFetch(...args);
      if (armed) {
        armed = false;
        // A separate document can finish logout in this gap. Queue it between
        // the real fetch guard and its caller, without modifying either module.
        void pending.then(() => queueMicrotask(() => {
          try {
            const xhr = new XMLHttpRequest();
            xhr.open('POST', '/api/auth/logout', false);
            xhr.setRequestHeader('X-CSRF-Token', initial.csrf_token);
            xhr.setRequestHeader('Content-Type', 'application/json');
            xhr.send('{}');
            logoutStatus = xhr.status;
            http.clearAuthToken();
            sync.announceAuthChange();
          } catch (error) { schedulerError = error instanceof Error ? error.name : 'scheduler failure'; }
        }));
      }
      return pending;
    };
    try {
      const response = await http.apiRequest('/api/auth/me');
      return { rejected: false, returnedPriorUser: response.user?.id === initial.user.id, logoutStatus, schedulerError };
    } catch (error) {
      return { rejected: (error as { status?: number }).status === 0, returnedPriorUser: false, logoutStatus, schedulerError };
    } finally { window.fetch = nativeFetch; }
  });
  const afterLogout = (await page.request.get('/api/auth/me')).status();
  await test.info().attach('api-response-session-boundary', { body: JSON.stringify({ ...result, afterLogout }), contentType: 'application/json' });
  expect(result.schedulerError).toBe('');
  expect(result.logoutStatus).toBe(200);
  expect(afterLogout).toBe(401);
  expect(result.rejected).toBe(true);
  expect(result.returnedPriorUser).toBe(false);
});
