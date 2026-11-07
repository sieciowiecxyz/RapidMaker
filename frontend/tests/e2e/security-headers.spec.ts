import { expect, test } from '@playwright/test';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';

const policy = "frame-ancestors 'none'; object-src 'none'; base-uri 'self'";

test('all HTML entry points return the explicit security headers', async ({
  request,
}, testInfo) => {
  const entries = [];
  for (const path of [
    '/index.html',
    '/admin.html',
    '/admin-design.html',
    '/checkout-design.html',
  ]) {
    const response = await request.get(path);
    entries.push({ path, status: response.status(), headers: response.headers() });
  }
  await testInfo.attach('html-response-headers', {
    body: JSON.stringify(entries, null, 2),
    contentType: 'application/json',
  });
  for (const entry of entries) {
    expect.soft(entry.status, entry.path).toBe(200);
    expect.soft(entry.headers['content-type'], entry.path).toContain('text/html');
    expect.soft(entry.headers['content-security-policy'], entry.path).toBe(policy);
    expect.soft(entry.headers['x-frame-options'], entry.path).toBe('DENY');
    expect.soft(entry.headers['x-content-type-options'], entry.path).toBe('nosniff');
  }
});

test('Chromium refuses a cross-origin parent framing the public app', async ({
  page,
  request,
  baseURL,
}, testInfo) => {
  const target = new URL('/index.html', baseURL).href;
  const direct = await request.get(target);
  expect(direct.status()).toBe(200);
  const refusals: string[] = [];
  const targetStatuses: number[] = [];
  page.on('console', (message) => {
    if (/refused to (?:frame|display)|framing.*violates/i.test(message.text()))
      refusals.push(message.text());
  });
  page.on('response', (response) => {
    if (response.url() === target) targetStatuses.push(response.status());
  });
  const parent = createServer((_request, response) => {
    response.writeHead(200, { 'Content-Type': 'text/html' });
    response.end(`<iframe src="${target}" width="1440" height="1000"></iframe>`);
  });
  let framedAppVisible = false;
  try {
    await new Promise<void>((resolve) => parent.listen(0, '127.0.0.1', resolve));
    const parentURL = `http://127.0.0.1:${(parent.address() as AddressInfo).port}`;
    expect(new URL(parentURL).origin).not.toBe(new URL(target).origin);
    await page.goto(parentURL);
    const app = page.frameLocator('iframe').locator('#app');
    // Record the unprotected baseline as well as a positive browser refusal.
    await expect.poll(async () => refusals.length > 0 || (await app.isVisible())).toBe(true);
    framedAppVisible = await app.isVisible();
    expect(targetStatuses).toContain(200);
    expect(refusals.length, 'Chromium must report a framing policy refusal').toBeGreaterThan(0);
    expect(framedAppVisible, 'the cross-origin child must not expose the app').toBe(false);
    await expect(app).toHaveCount(0);
  } finally {
    await testInfo.attach('browser-framing-evidence', {
      body: JSON.stringify(
        { directStatus: direct.status(), targetStatuses, framedAppVisible, refusals },
        null,
        2,
      ),
      contentType: 'application/json',
    });
    parent.closeAllConnections();
    await new Promise<void>((resolve, reject) =>
      parent.close((error) => (error ? reject(error) : resolve())),
    );
  }
});
