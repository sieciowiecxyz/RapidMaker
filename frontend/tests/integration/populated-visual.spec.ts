import { expect, test, type Page } from './fixtures';
import { accounts, login } from './helpers';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { randomUUID } from 'node:crypto';
import { execFileSync } from 'node:child_process';

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
  test.setTimeout(300_000);
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
  const capture = async (target: Page, label: string, alerts = 0) => {
    await expect(target.getByRole('alert')).toHaveCount(alerts);
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
    const catalog = await (await operator.request.get('/api/admin/materials')).json();
    const material = catalog.materials.find((material: { id: string }) => material.id === 'GFA00');
    const chosenColor = material.colors[0];
    const color = chosenColor.code;
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
    // Names belong to the quote snapshot, even after the catalog is edited.
    const materialInput = {
      name: material.name,
      density_g_cm3: material.density_g_cm3,
      cost_net_grosze_per_kg: material.cost_net_grosze_per_kg,
      production_enabled: material.production_enabled,
    };
    const materialUrl = '/api/admin/materials/GFA00';
    const colorUrl = `${materialUrl}/colors/${encodeURIComponent(color)}`;
    const adminHeaders = await headers(operator);
    const database = process.env.RAPIDMAKER_TEST_DATABASE ?? '';
    expect(database).toMatch(/^rapidmaker_test_browser_[a-f0-9]+$/);
    const sql = (query: string) =>
      execFileSync(
        'podman',
        [
          'exec',
          process.env.TEST_PG_CONTAINER ?? 'rapidmaker-catalog-postgres',
          'psql',
          '-U',
          'rapidmaker',
          '-d',
          database,
          '-v',
          'ON_ERROR_STOP=1',
          '-c',
          query,
        ],
        { stdio: 'pipe' },
      );
    try {
      expect(
        (
          await operator.request.put(materialUrl, {
            headers: adminHeaders,
            data: { ...materialInput, name: 'Renamed material for snapshot check' },
          })
        ).status(),
      ).toBe(200);
      expect(
        (
          await operator.request.put(colorUrl, {
            headers: adminHeaders,
            data: { ...chosenColor, name: 'Renamed color for snapshot check' },
          })
        ).status(),
      ).toBe(200);
      for (const [target, prefix] of [
        [page, 'account'],
        [operator, 'admin'],
      ] as const) {
        const response = await target.request.get(`/api/${prefix}/orders/${id}/files`);
        expect(response.status()).toBe(200);
        const data = await response.json();
        expect(data.files).toHaveLength(1);
        expect(data.files[0]).toMatchObject({
          material_id: 'GFA00',
          color_code: color,
          material_name: material.name,
          color_name: chosenColor.name,
          options: { quality: 'standard', wall_loops: 3, infill_percent: 20 },
        });
      }
      const file = (await (await page.request.get(`/api/account/orders/${id}/files`)).json())
        .files[0];
      expect(file.quote_id).toMatch(
        /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/,
      );
      const hexText = (text: string) =>
        `convert_from(decode('${Buffer.from(text).toString('hex')}','hex'),'UTF8')`;
      try {
        // Simulate old snapshots only in the disposable DB; never weaken the public write API.
        sql(
          `UPDATE quotes SET request=request-'material_name'-'color_name' WHERE id='${file.quote_id}'`,
        );
        const legacy = (await (await page.request.get(`/api/account/orders/${id}/files`)).json())
          .files;
        expect(legacy).toHaveLength(1);
        expect(legacy[0]).toMatchObject({
          material_name: 'Renamed material for snapshot check',
          color_name: 'Renamed color for snapshot check',
        });
        sql(
          `UPDATE quotes SET request=request||jsonb_build_object('material_id','missing-test-material','color_code','missing-test-color') WHERE id='${file.quote_id}'`,
        );
        const missing = (await (await page.request.get(`/api/account/orders/${id}/files`)).json())
          .files;
        expect(missing).toHaveLength(1);
        expect(missing[0]).toMatchObject({
          material_id: 'missing-test-material',
          color_code: 'missing-test-color',
          material_name: null,
          color_name: null,
          available: true,
        });
        expect((await page.request.get(missing[0].preview_url)).status()).toBe(200);
      } finally {
        sql(
          `UPDATE quotes SET request=request||jsonb_build_object('material_id','GFA00','color_code',${hexText(color)},'material_name',${hexText(material.name)},'color_name',${hexText(chosenColor.name)}) WHERE id='${file.quote_id}'`,
        );
      }
    } finally {
      // Always restore both records, including when a regression assertion fails.
      const restored = await Promise.all([
        operator.request.put(materialUrl, { headers: adminHeaders, data: materialInput }),
        operator.request.put(colorUrl, { headers: adminHeaders, data: chosenColor }),
      ]);
      expect(restored.map((response) => response.status())).toEqual([200, 200]);
    }
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
        await expect(
          page
            .locator('article.model')
            .getByText(`${material.name} · ${chosenColor.name}`, { exact: true }),
        ).toBeVisible();
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
        await expect(drawer.locator('.material-tag')).toHaveText(material.name);
        await expect(drawer.locator('.color-label')).toHaveText(chosenColor.name);
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
        await expect(
          operator
            .locator('article.model')
            .getByText(`${material.name} · ${chosenColor.name}`, { exact: true }),
        ).toBeVisible();
        await preview(operator, `${label}-admin-detail`);
        await capture(operator, `${label}-admin-detail`);
        await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
        await expect(operator.locator('html')).toHaveAttribute('data-theme', theme);
      }
    }
    // Preserve the awaiting-quote evidence before publishing a real, still-unpaid offer.
    const order = await (await page.request.get(`/api/account/orders/${id}`)).json();
    expect(order.manual_offer.state).toBe('awaiting_quote');
    expect(order.amount_grosze).toBeNull();
    expect(order).toMatchObject({
      status: 'awaiting_quote',
      paid_at: null,
      payment_attempt_id: null,
    });
    const scope =
      'Wydruk trzech części PLA według załączonego modelu; trzy ściany i wypełnienie 20%. ' +
      'Usunięcie podpór i kontrola wymiarów przed odbiorem osobistym. Termin do 14 dni roboczych ' +
      'od potwierdzenia zamówienia. Cena obejmuje cały opisany zakres, bez dodatkowych usług.';
    const publish = operator.getByRole('button', {
      name: 'Opublikuj ofertę i powiadom klienta',
      exact: true,
    });
    const matrix: { width: number; theme: string; state: string; role: string; image: string }[] =
      [];
    const offerCard = (target: Page) =>
      target.getByRole('region', { name: 'Wycena indywidualna', exact: true });
    const offerSnapshot = async (
      target: Page,
      width: number,
      theme: string,
      state: string,
      role: string,
      alerts = 0,
    ) => {
      const label = `${width}-${theme}-${state}-${role}-offer`;
      await expect(target.getByRole('alert')).toHaveCount(alerts);
      await target.evaluate(() => document.fonts.ready);
      const card = offerCard(target);
      await card.scrollIntoViewIfNeeded();
      // Every control must be usable without horizontal scrolling, including narrow layouts.
      for (const control of await card.locator('header, input, textarea, button, a').all()) {
        await control.scrollIntoViewIfNeeded();
        const bounds = (await control.boundingBox())!;
        expect(bounds.x, label).toBeGreaterThanOrEqual(0);
        expect(bounds.x + bounds.width, label).toBeLessThanOrEqual(width + 1);
        expect(bounds.y, label).toBeGreaterThanOrEqual(0);
        expect(bounds.y + bounds.height, label).toBeLessThanOrEqual(901);
      }
      await card.screenshot({ path: info.outputPath(`${label}-card.png`) });
      expect(await target.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
        true,
      );
      matrix.push({ width, theme, state, role, image: `${label}-card.png` });
      writeFileSync(info.outputPath('manual-offer-matrix.json'), JSON.stringify(matrix, null, 2));
    };
    const offerCells = async (state: 'invalid' | 'published' | 'expired') => {
      for (const width of [1440, 390]) {
        for (const theme of ['light', 'dark']) {
          for (const [target, role, route] of [
            [page, 'client', `/konto/zamowienia/${id}`],
            [operator, 'admin', `/admin/zamowienia/${id}`],
          ] as const) {
            await target.setViewportSize({ width, height: 900 });
            await target.goto('/');
            if ((await target.locator('html').getAttribute('data-theme')) !== theme)
              await target
                .getByRole('button', {
                  name: theme === 'light' ? 'Zmień na jasny motyw' : 'Zmień na ciemny motyw',
                  exact: true,
                })
                .click();
            await target.goto(route);
            const card = offerCard(target);
            await expect(card).toBeVisible();
            if (state === 'invalid') {
              if (role === 'admin') {
                // Both zero total and shipping above the total must fail inline, before publication.
                for (const [total, shipping] of [
                  ['0,00', '0,00'],
                  ['25,00', '25,01'],
                ]) {
                  await target.getByLabel('Zakres wykonania i dostawy').fill(scope);
                  await target.getByLabel('Całość brutto (zł)').fill(total);
                  await target.getByLabel('W tym dostawa (zł)').fill(shipping);
                  await publish.click();
                  await expect(card.getByRole('alert')).toHaveText(
                    'Podaj dodatnią cenę całkowitą, dostawę nie większą od ceny i opis zakresu.',
                  );
                  await expect(target.getByLabel('Całość brutto (zł)')).toHaveValue(total);
                  await expect(target.getByLabel('W tym dostawa (zł)')).toHaveValue(shipping);
                  await expect(target.getByLabel('Zakres wykonania i dostawy')).toHaveValue(scope);
                  const unchanged = await (
                    await page.request.get(`/api/account/orders/${id}`)
                  ).json();
                  expect(unchanged.manual_offer).toMatchObject({
                    state: 'awaiting_quote',
                    version: order.manual_offer.version,
                    amount_grosze: null,
                  });
                  expect(unchanged.payment_attempt_id).toBeNull();
                  await offerSnapshot(
                    target,
                    width,
                    theme,
                    total === '0,00' ? 'invalid-amount' : 'invalid-delivery',
                    role,
                    1,
                  );
                }
                continue;
              } else {
                await expect(
                  card.getByRole('heading', { name: 'Oczekuje na wycenę' }),
                ).toBeVisible();
                await expect(
                  card.getByRole('button', { name: 'Przygotuj płatność za ofertę' }),
                ).toHaveCount(0);
              }
            } else {
              await expect(card.locator('.offer-amount')).toContainText('25,00');
              await expect(card.locator('.offer-amount small')).toContainText('0,00');
              await expect(card.getByText(scope, { exact: true })).toBeVisible();
              const pay = card.getByRole('button', {
                name: 'Przygotuj płatność za ofertę',
                exact: true,
              });
              if (state === 'published') {
                if (role === 'client') await expect(pay).toBeEnabled();
                else {
                  await expect(target.getByLabel('Całość brutto (zł)')).toHaveValue('25,00');
                  await expect(target.getByLabel('W tym dostawa (zł)')).toHaveValue('0,00');
                  await expect(publish).toBeEnabled();
                }
              } else {
                await expect(pay).toHaveCount(0);
                await expect(card.getByRole('textbox')).toHaveCount(0);
                await expect(
                  card.getByRole('button', { name: 'Opublikuj ofertę i powiadom klienta' }),
                ).toHaveCount(0);
                await expect(
                  card.getByText(
                    role === 'admin'
                      ? /Oferta wygasła\. Potrzebne/
                      : /Ta oferta lub próba płatności wygasła/,
                  ),
                ).toBeVisible();
              }
            }
            await expect(target.locator('html')).toHaveAttribute('data-theme', theme);
            await offerSnapshot(target, width, theme, state, role);
          }
        }
      }
    };
    await offerCells('invalid');
    await operator.goto(`/admin/zamowienia/${id}`);
    await operator.getByLabel('Całość brutto (zł)').fill('25,00');
    await operator.getByLabel('W tym dostawa (zł)').fill('0,00');
    await operator.getByLabel('Zakres wykonania i dostawy').fill(scope);
    operator.once('dialog', async (dialog) => {
      expect(dialog.type()).toBe('confirm');
      await dialog.accept();
    });
    const publication = operator.waitForResponse(
      (response) =>
        response.url().endsWith(`/api/admin/orders/${id}/quote`) &&
        response.request().method() === 'POST',
    );
    await publish.click();
    expect((await publication).status()).toBe(200);
    await expect
      .poll(
        async () =>
          (await (await page.request.get(`/api/account/orders/${id}`)).json()).manual_offer.state,
      )
      .toBe('offered');
    const offered = await (await page.request.get(`/api/account/orders/${id}`)).json();
    expect(offered.manual_offer).toMatchObject({
      state: 'offered',
      amount_grosze: 2500,
      shipping_grosze: 0,
      scope,
      payment_started: false,
    });
    expect(offered).toMatchObject({ amount_grosze: 2500, paid_at: null, payment_attempt_id: null });
    await offerCells('published');

    // Expire only this synthetic order in the explicitly validated disposable database.
    expect(id).toMatch(/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/);
    expect(database).toMatch(/^rapidmaker_test_browser_[a-f0-9]+$/);
    sql(
      `UPDATE manual_quotes SET expires_at=extract(epoch from now())::bigint-60 WHERE order_id='${id}'`,
    );
    for (const [target, prefix] of [
      [page, 'account'],
      [operator, 'admin'],
    ] as const) {
      const response = await target.request.get(
        prefix === 'admin' ? '/api/admin/orders' : `/api/account/orders/${id}`,
      );
      expect(response.status()).toBe(200);
      const data = await response.json();
      const expired =
        prefix === 'admin' ? data.orders.find((order: { id: string }) => order.id === id) : data;
      expect(expired).toBeDefined();
      expect(expired.manual_offer).toMatchObject({
        state: 'expired',
        amount_grosze: 2500,
        shipping_grosze: 0,
        scope,
        payment_started: false,
      });
      expect(expired).toMatchObject({ paid_at: null, payment_attempt_id: null });
    }
    await offerCells('expired');
    // The HTTP payment endpoint checks Stripe configuration before expiry. Providers stay disabled;
    // this test proves server-read expiry and unavailable UI actions, not expired-payment HTTP rejection.
    info.annotations.push({
      type: 'limitation',
      description:
        'Expired-payment HTTP rejection is not exercised with providers disabled; no payment was initiated.',
    });
    expect(matrix).toHaveLength(28);
    expect(operatorErrors).toEqual([]);
  } finally {
    await adminContext.close();
  }
});
