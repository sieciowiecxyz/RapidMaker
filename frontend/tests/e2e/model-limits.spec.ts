import { expect, test } from '@playwright/test';
import { resolve } from 'node:path';

test.use({ screenshot: 'off', trace: 'off', reducedMotion: 'reduce' });
const MiB = 1024 * 1024;
const unavailable = 'Test: analiza API jest niedostępna.';

// Controlled UI contracts: metadata and uploads are stubbed; no server geometry is claimed.
for (const limit of [32 * MiB, 96 * MiB, 0, 128 * MiB, '33554432', null, 'offline']) {
  test(`upload limit ${typeof limit}:${JSON.stringify(limit)} controls file admission`, async ({
    page,
  }) => {
    const valid = limit === 32 * MiB || limit === 96 * MiB;
    const cap = valid ? Number(limit) / MiB : 25;
    let metadataGets = 0,
      uploads = 0;
    await page.route('**/api/**', async (route) => {
      const request = route.request();
      const path = new URL(request.url()).pathname;
      if (path === '/api/auth/me') return route.fulfill({ status: 401, json: {} });
      if (path === '/api/models/limits') {
        expect(request.method()).toBe('GET');
        metadataGets++;
        return limit === 'offline'
          ? route.abort('connectionfailed')
          : route.fulfill({ json: { max_upload_bytes: limit } });
      }
      if (path === '/api/models' && request.method() === 'POST') uploads++;
      return route.fulfill({ status: 503, json: { message: unavailable } });
    });
    await page.goto('/wycena');
    await expect.poll(() => metadataGets).toBe(1);
    await expect(
      page.getByText(`STL, STEP, 3MF / do ${cap} MB na plik`, { exact: true }),
    ).toBeVisible();
    await page.getByLabel('Dodaj modele STL, STEP lub 3MF').setInputFiles({
      name: 'boundary.3mf',
      mimeType: 'application/octet-stream',
      buffer: Buffer.alloc(25 * MiB + 1),
    });
    if (valid) {
      await expect(page.getByRole('alert')).toHaveText(unavailable);
      expect(uploads).toBe(1);
      expect(metadataGets).toBe(1); // The live hint and admission reuse one cached metadata request.
    } else {
      await expect(page.getByRole('alert')).toHaveText('Wybierz niepusty plik do 25 MB.');
      expect(uploads).toBe(0);
      await expect(page.locator('.quote-contact')).toBeDisabled();
    }
    if (limit === 'offline') {
      await page
        .getByLabel('Dodaj modele STL, STEP lub 3MF')
        .setInputFiles(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
      await expect(page.getByRole('alert')).toHaveText(unavailable);
      expect(uploads).toBe(1);
      await expect(
        page.getByRole('img', { name: /Model z pliku cube_20mm.stl.*Czarny/ }),
      ).toBeVisible();
      await expect(page.locator('.print-scene .scene-caption')).toContainText('12 trójkątów');
    }
  });
}
