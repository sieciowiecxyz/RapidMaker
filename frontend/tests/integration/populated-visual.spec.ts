import { expect, test, type Page } from './fixtures';
import { accounts, login } from './helpers';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { randomUUID } from 'node:crypto';

// Login codes and secrets are never captured. Later screenshots contain synthetic order data only.
test.use({ screenshot: 'off', trace: 'off' });

test('populated unpaid order, private previews and chat fit both themes and viewports', async ({
  page,
  browser,
}, info) => {
  test.skip(
    !process.env.RAPIDMAKER_TEST_ACCOUNTS || process.env.RAPIDMAKER_SANDBOX === '1',
    'Requires --local-accounts with providers disabled',
  );
  test.setTimeout(180_000);
  const client = accounts().find((account) => account.email === 'browser-other@example.invalid')!;
  const admin = accounts().find((account) => account.role === 'admin')!;
  const adminContext = await browser.newContext();
  const operator = await adminContext.newPage();
  const operatorErrors: string[] = [];
  operator.on('pageerror', (error) => operatorErrors.push(error.name));
  const headers = async (target: Page) => {
    const session = await (await target.request.get('/api/auth/me')).json();
    expect(session.stage).toBe('authenticated');
    return { Origin: 'http://localhost:18437', 'X-CSRF-Token': session.csrf_token };
  };
  const capture = async (target: Page, label: string) => {
    await expect(target.getByRole('alert')).toHaveCount(0);
    await target.evaluate(() => document.fonts.ready);
    for (const image of await target.locator('img').all())
      await expect
        .poll(() =>
          image.evaluate(
            (node) => node instanceof HTMLImageElement && node.complete && node.naturalWidth > 0,
          ),
        )
        .toBe(true);
    expect(await target.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
      true,
    );
    await target.evaluate(() => scrollTo({ top: 0, behavior: 'instant' }));
    await target.screenshot({ path: info.outputPath(`${label}.png`) });
  };
  const preview = async (target: Page, label: string) => {
    const canvas = target.getByLabel('Podgląd modelu cube_20mm.stl', { exact: true });
    await expect(canvas).toBeVisible();
    await expect(target.locator('.model-preview')).toHaveAttribute('aria-busy', 'false');
    const rotate = target.getByRole('button', { name: 'Obróć model w prawo', exact: true });
    await expect(rotate).toBeVisible(); // Tools appear only after GLB parsing and WebGL setup succeed.
    const before = await canvas.screenshot();
    await rotate.click();
    await expect.poll(async () => !(await canvas.screenshot()).equals(before)).toBe(true);
    await canvas.screenshot({ path: info.outputPath(`${label}-preview.png`) });
  };
  try {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await operator.emulateMedia({ reducedMotion: 'reduce' });
    await login(page, client);
    await login(operator, admin);
    const upload = await page.request.post('/api/models', {
      headers: {
        ...(await headers(page)),
        'Content-Type': 'application/octet-stream',
        'x-model-filename': 'cube_20mm.stl',
      },
      data: readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl')),
    });
    expect(upload.status()).toBe(201);
    const model = await upload.json();
    const catalog = await (await page.request.get('/api/catalog/materials')).json();
    const color = catalog.materials.find((material: { id: string }) => material.id === 'GFA00')
      .colors[0].code;
    const created = await page.request.post('/api/account/manual-quotes', {
      headers: await headers(page),
      data: {
        request_id: randomUUID(),
        parts: [
          {
            model_id: model.id,
            quantity: 3,
            material_id: 'GFA00',
            color_code: color,
            options: { quality: 'standard', wall_loops: 3, infill_percent: 20 },
          },
        ],
        contact: {
          name: 'Test widoku zamówienia',
          email: client.email,
          phone: '500600700',
          country: 'PL',
          street: '',
          apartment: '',
          city: '',
          postal_code: '',
          invoice: null,
        },
        delivery: { method: 'pickup' },
        notes: 'Test wizualny: trzy części, odbiór osobisty. Bez płatności.',
      },
    });
    expect(created.status()).toBe(200);
    const { order_id: id } = await created.json();
    expect(id).toMatch(/^[a-f0-9-]{36}$/);
    const number = `RM-${id.slice(0, 8).toUpperCase()}`;
    const clientMessage = 'Proszę przygotować trzy jednakowe części.';
    const reply = 'Sprawdzimy plik i prześlemy indywidualną wycenę.';
    await page.goto(`/konto/zamowienia/${id}`);
    await expect(page.getByRole('heading', { level: 1, name: number, exact: true })).toBeVisible();
    await page.getByLabel('Wiadomość', { exact: true }).fill(clientMessage);
    await page.getByRole('button', { name: 'Wyślij wiadomość', exact: true }).click();
    await expect(page.getByText(clientMessage, { exact: true })).toBeVisible();
    await operator.goto(`/admin/zamowienia/${id}`);
    await operator.getByLabel('Wiadomość', { exact: true }).fill(reply);
    await operator.getByRole('button', { name: 'Wyślij wiadomość', exact: true }).click();
    await expect(operator.getByText(reply, { exact: true })).toBeVisible();
    await page.reload();
    await expect(page.getByText(reply, { exact: true })).toBeVisible();

    for (const width of [1440, 390]) {
      await page.setViewportSize({ width, height: 900 });
      await operator.setViewportSize({ width, height: 900 });
      for (const theme of ['light', 'dark']) {
        const label = `${width}-${theme}`;
        // Each context owns its preference; use the real storefront control instead of setting DOM state.
        for (const target of [page, operator]) {
          await target.goto('/');
          if ((await target.locator('html').getAttribute('data-theme')) !== theme)
            await target
              .getByRole('button', {
                name: theme === 'light' ? 'Zmień na jasny motyw' : 'Zmień na ciemny motyw',
                exact: true,
              })
              .click();
        }
        await page.goto('/konto/zamowienia');
        await expect(
          page.getByRole('heading', { name: 'Moje zamówienia', exact: true }),
        ).toBeVisible();
        await expect(
          page.getByRole('link', { name: `Szczegóły zamówienia ${number}`, exact: true }),
        ).toHaveAttribute('href', `/konto/zamowienia/${id}`);
        await expect(page.getByText('cube_20mm.stl', { exact: true })).toBeVisible();
        await expect(
          page.getByRole('listitem', { name: number, exact: true }).locator('.price'),
        ).toHaveText('Do wyceny');
        await capture(page, `${label}-client-list`);
        await page
          .getByRole('link', { name: `Szczegóły zamówienia ${number}`, exact: true })
          .click();
        await expect(
          page.getByRole('heading', { level: 1, name: number, exact: true }),
        ).toBeVisible();
        await expect(
          page.getByRole('heading', { name: 'Oczekuje na wycenę', exact: true }),
        ).toBeVisible();
        const colors = await page
          .locator('.page-heading .badge.attention')
          .evaluate((badge) => [
            getComputedStyle(badge).color,
            getComputedStyle(document.body).backgroundColor,
          ]);
        const luminance = (color: string) => {
          const rgb = color
            .match(/[\d.]+/g)!
            .slice(0, 3)
            .map((value) => {
              const channel = Number(value) / 255;
              return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
            });
          return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
        };
        const levels = colors.map(luminance);
        const contrast = (Math.max(...levels) + 0.05) / (Math.min(...levels) + 0.05);
        expect(
          contrast,
          `${label}: manual quote label must remain readable`,
        ).toBeGreaterThanOrEqual(4.5);
        info.annotations.push({
          type: 'status-contrast',
          description: `${label}: ${contrast.toFixed(2)}`,
        });
        await preview(page, `${label}-client-detail`);
        await capture(page, `${label}-client-detail`);
        const chat = page.getByRole('region', { name: 'Rozmowa o zamówieniu', exact: true });
        await expect(chat.getByText(clientMessage, { exact: true })).toBeVisible();
        await expect(chat.getByText(reply, { exact: true })).toBeVisible();
        await chat.scrollIntoViewIfNeeded();
        await chat.screenshot({ path: info.outputPath(`${label}-client-chat.png`) });

        await operator.goto('/admin.html');
        await expect(
          operator.getByRole('heading', { level: 1, name: 'Zamówienia', exact: true }),
        ).toBeVisible();
        const open = operator.getByRole('button', { name: `Otwórz ${id}`, exact: true });
        await expect(open).toBeVisible();
        await expect(
          operator.getByRole('row').filter({ has: open }).locator('td.amount'),
        ).toHaveText('Do wyceny');
        await capture(operator, `${label}-admin-list`);
        await open.click();
        const drawer = operator.getByRole('complementary', {
          name: `Szczegóły ${id}`,
          exact: true,
        });
        await expect(drawer).toBeVisible();
        const rect = await drawer.boundingBox();
        expect(rect!.x).toBeGreaterThanOrEqual(0);
        expect(rect!.x + rect!.width).toBeLessThanOrEqual(width + 1);
        const tabs = drawer.getByLabel('Sekcje zamówienia', { exact: true });
        await tabs.getByRole('button', { name: 'Modele i wycena', exact: true }).focus();
        for (let step = 0; step < 6; step++) await operator.keyboard.press('Tab');
        const history = tabs.getByRole('button', { name: 'Historia', exact: true });
        await expect(history).toBeFocused();
        await operator.keyboard.press('Enter');
        await expect(
          drawer.getByRole('heading', { name: 'Historia realizacji', exact: true }),
        ).toBeVisible();
        const tabBounds = (await tabs.boundingBox())!;
        const historyBounds = (await history.boundingBox())!;
        expect(historyBounds.x).toBeGreaterThanOrEqual(tabBounds.x);
        expect(historyBounds.x + historyBounds.width).toBeLessThanOrEqual(
          tabBounds.x + tabBounds.width + 1,
        );
        await capture(operator, `${label}-admin-history`);
        await drawer.getByRole('button', { name: 'Pliki', exact: true }).click();
        await preview(operator, `${label}-admin-files`);
        await capture(operator, `${label}-admin-files`);
        await drawer.getByRole('button', { name: 'Wiadomości', exact: true }).click();
        await expect(drawer.getByText(clientMessage, { exact: true })).toBeVisible();
        await expect(drawer.getByText(reply, { exact: true })).toBeVisible();
        await capture(operator, `${label}-admin-chat`);
        await operator.goto(`/admin/zamowienia/${id}`);
        await expect(
          operator.getByRole('heading', { level: 1, name: number, exact: true }),
        ).toBeVisible();
        await expect(operator.getByLabel('Całość brutto (zł)')).toBeEnabled();
        await preview(operator, `${label}-admin-detail`);
        await capture(operator, `${label}-admin-detail`);
        await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
        await expect(operator.locator('html')).toHaveAttribute('data-theme', theme);
      }
    }
    // This order stays a genuine unpaid request: no payment/provider result was fabricated.
    const order = await (await page.request.get(`/api/account/orders/${id}`)).json();
    expect(order.manual_offer.state).toBe('awaiting_quote');
    expect(order.amount_grosze).toBeNull();
    expect(order).toMatchObject({
      status: 'awaiting_quote',
      paid_at: null,
      payment_attempt_id: null,
    });
    expect(operatorErrors).toEqual([]);
  } finally {
    await adminContext.close();
  }
});
