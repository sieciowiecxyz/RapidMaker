import { expect, test } from '@playwright/test';

test('real legal documents are public, downloadable and stale enrollment is rejected', async ({
  page,
}, info) => {
  const documents = await page.request.get('/api/legal/documents');
  expect(documents.status()).toBe(200);
  const legal = await documents.json();
  expect(legal.version).toBe('2026-10-25');
  expect(legal.terms).toContain('1231537290');
  expect(legal.privacy).toContain('piotr@rapidmaker.pl');
  const external: string[] = [];
  page.on('request', (request) => {
    if (!new URL(request.url()).hostname.match(/^(localhost|127\.0\.0\.1)$/))
      external.push(request.url());
  });
  await page.goto('/regulaminy');
  await expect(page.getByRole('heading', { name: 'Dokumenty i zasady' })).toBeVisible();
  expect(external).toEqual([]);
  await page.screenshot({ path: info.outputPath('legal-desktop.png'), fullPage: true });
  await page.getByRole('link', { name: /Regulamin Zamówienia/ }).click();
  await expect(
    page.getByRole('heading', { name: '5. Odstąpienie od zakupu i reklamacje' }),
  ).toBeVisible();
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: /Pobierz dokument/ }).click();
  expect((await download).suggestedFilename()).toBe('rapidmaker-regulamin-2026-10-25.txt');
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: info.outputPath('terms-mobile.png'), fullPage: false });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  for (const legal_version of [undefined, 'old-version']) {
    const response = await page.request.post('/api/auth/start', {
      headers: { Origin: new URL(page.url()).origin },
      data: { email: 'nobody@example.invalid', legal_version },
    });
    expect(response.status()).toBe(409);
  }
});
