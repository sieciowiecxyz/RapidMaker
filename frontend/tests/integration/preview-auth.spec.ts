import { expect, test } from './fixtures';
import { accounts, login } from './helpers';
import { resolve } from 'node:path';

test.use({ screenshot: 'off', trace: 'off', reducedMotion: 'reduce' });
type PreviewGate = { received: boolean; release: () => void };

test('a private GLB body cannot populate the preview after another tab logs out', async ({
  page,
  context,
}) => {
  test.skip(!process.env.RAPIDMAKER_TEST_ACCOUNTS, 'Requires --local-accounts');
  const account = accounts().find(
    (account) => account.email === 'browser-security@example.invalid',
  );
  expect(account?.totp_uri).toBeTruthy();
  await login(page, account!);
  // Real API and real GLB; hold only the browser body reader before its caller resumes.
  await page.addInitScript(() => {
    let release!: () => void;
    const gate = new Promise<void>((done) => {
      release = done;
    });
    const state = { received: false, release };
    (window as unknown as { previewGate: PreviewGate }).previewGate = state;
    const read = Response.prototype.arrayBuffer;
    Response.prototype.arrayBuffer = async function () {
      const buffer = await read.call(this);
      if (this.ok && new URL(this.url).pathname.endsWith('/preview.glb')) {
        state.received = true;
        await gate;
      }
      return buffer;
    };
  });
  await page.goto('/wycena');
  await page
    .getByLabel('Dodaj modele STL, STEP lub 3MF')
    .setInputFiles(resolve('../backend/crates/geometry/tests/fixtures/step_block.step'));
  await expect
    .poll(() =>
      page.evaluate(() => (window as unknown as { previewGate: PreviewGate }).previewGate.received),
    )
    .toBe(true);
  const id = await page.evaluate(
    () => JSON.parse(localStorage.getItem('rapidmaker.storefront.draft.v1')!).models[0].analysis.id,
  );
  const other = await context.newPage();
  await other.goto('/konto/zamowienia');
  const logout = other.waitForResponse((r) => new URL(r.url()).pathname === '/api/auth/logout');
  await other.getByRole('button', { name: 'Wyloguj', exact: true }).click();
  const logoutStatus = (await logout).status();
  const privateReadStatus = (await page.request.get(`/api/models/${id}/preview.glb`)).status();
  await page.evaluate(() =>
    (window as unknown as { previewGate: PreviewGate }).previewGate.release(),
  );
  const displayed = await Promise.race([
    page
      .getByRole('img', { name: /^Model z pliku/ })
      .waitFor({ state: 'visible' })
      .then(() => true),
    page
      .getByRole('heading', { name: 'Nie można wyświetlić modelu' })
      .waitFor({ state: 'visible' })
      .then(() => false),
  ]);
  await test.info().attach('private-preview-session-boundary', {
    body: JSON.stringify({ logoutStatus, privateReadStatus, displayed }),
    contentType: 'application/json',
  });
  expect(logoutStatus).toBe(200);
  expect(privateReadStatus).toBe(401);
  expect(displayed).toBe(false);
  await page.getByRole('button', { name: 'Usuń step_block.step', exact: true }).click();
  await expect(page.locator('.model-row')).toHaveCount(0);
});

test('a local STL still renders when the upload fails and another tab changes the auth signal', async ({
  page,
  context,
}) => {
  await page.goto('/wycena');
  await page.route('**/api/models', (route) => route.abort('failed'));
  await page
    .getByLabel('Dodaj modele STL, STEP lub 3MF')
    .setInputFiles(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
  const view = page.getByRole('img', { name: /^Model z pliku cube_20mm.stl/ });
  await expect(view).toBeVisible();
  const other = await context.newPage();
  await other.goto('/wycena');
  await other.evaluate(() => localStorage.setItem('rapidmaker:auth-change', crypto.randomUUID()));
  await page.evaluate(
    () =>
      new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done()))),
  );
  await expect(view).toBeVisible();
  await page.getByRole('button', { name: 'Obróć model', exact: true }).click();
  await expect(view).toBeVisible();
});
