import { expect, test, type Page, type Route } from '@playwright/test';
import { resolve } from 'node:path';

const modelId = '00000000-0000-4000-8000-000000000002';
async function configure(
  page: Page,
  configured = true,
  checkout?: (route: Route) => Promise<void>,
  identity?: () => { id: string; email: string },
  stopped = false,
) {
  const estimates: Record<string, unknown>[] = [];
  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === '/api/models/limits')
      return route.fulfill({ json: { max_upload_bytes: 32 * 1024 * 1024 } });
    if (path === '/api/auth/me')
      return identity
        ? route.fulfill({
            json: { user: { ...identity(), role: 'client' }, stage: 'authenticated' },
          })
        : route.fulfill({ status: 401, json: {} });
    if (path === '/api/checkout/config') return route.fulfill({ json: { publishable_key: null } });
    if (path === '/api/account/notifications')
      return route.fulfill({ json: { email_order_updates: true, email_chat_messages: true } });
    if (path === '/api/catalog/materials')
      return route.fulfill({
        json: {
          materials: [
            {
              id: 'GFG02',
              name: 'PETG',
              available: configured,
              colors: [{ code: '33102', name: 'Black', rgba: ['#000000FF'], in_stock: false }],
            },
          ],
        },
      });
    if (path.endsWith('/preview.glb')) return route.fulfill({ status: 404, body: '' });
    if (path === '/api/models')
      return route.fulfill({
        status: 201,
        json: {
          id: modelId,
          filename: 'cube_20mm.stl',
          dimensions_mm: [20, 20, 20],
          weight_g: null,
          weight_source: 'slicer',
        },
      });
    if (stopped && path === '/api/quotes/jobs/stopped-quote')
      return route.fulfill({
        json: {
          id: 'stopped-quote',
          status: 'failed',
          message: 'Automatyczna wycena została zatrzymana. Poproś o wycenę indywidualną.',
        },
      });
    if (path === '/api/quotes/estimate') {
      estimates.push(route.request().postDataJSON());
      if (stopped)
        return route.fulfill({ status: 202, json: { id: 'stopped-quote', status: 'queued' } });
      return configured
        ? route.fulfill({
            status: 202,
            json: {
              id: 'measured-quote',
              status: 'succeeded',
              result: { id: 'measured-quote', expires_at: Math.floor(Date.now() / 1000) + 600 },
            },
          })
        : route.fulfill({
            status: 422,
            json: {
              code: 'manual_quote_required',
              message: 'Materiał wymaga zatwierdzenia bezpiecznego profilu i jawnej ceny zakupu.',
            },
          });
    }
    if (path === '/api/checkout/quote') {
      if (checkout) return checkout(route);
      const parts = route.request().postDataJSON().parts as { id: string; quantity: number }[];
      return route.fulfill({
        json: {
          id: 'cart-quote',
          expires_at: Math.floor(Date.now() / 1000) + 600,
          packing: { status: 'estimated', parcels: [], packaging_grosze: 0 },
          offers: [],
          print_grosze: 1200 + parts.reduce((sum, part) => sum + 2450 * part.quantity, 0),
          print_lines: parts.map((part) => ({
            model_id: part.id,
            quantity: part.quantity,
            unit_grosze: 2450,
            total_grosze: 2450 * part.quantity,
          })),
          setup_grosze: 1200,
          packaging_grosze: 0,
        },
      });
    }
    throw new Error('Unexpected API: ' + path);
  });
  await page.goto('/wycena');
  await page
    .getByLabel('Dodaj modele STL, STEP lub 3MF')
    .setInputFiles(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
  await page.locator('input[name=material][value=petg]').check();
  await page.locator('input[name=quality][value=standard]').check();
  await page.locator('input[name=strength][value=standard]').check();
  return estimates;
}

test.use({ reducedMotion: 'reduce' });
test('a cross-family shade shows the measured material price but remains an unmodified manual request', async ({
  page,
}) => {
  const estimates = await configure(page);
  await page.locator('input[name=color][value=custom]').check();
  await page.getByRole('button', { name: 'PLA-CF', exact: true }).click();
  await page.getByRole('radio', { name: 'Matcha Green', exact: true }).check();
  const result = page.locator('.quote-result');
  await expect(result.getByText('Szacowana cena wydruku', { exact: true })).toBeVisible();
  await expect(result.locator('.price')).toContainText('36,50');
  await expect(result).toContainText('PETG wymaga potwierdzenia');
  await page.getByRole('button', { name: 'Powiel wariant cube_20mm.stl', exact: true }).click();
  await expect(
    page.getByText('Identyczny wariant. Zmień ustawienia lub zwiększ liczbę sztuk.', {
      exact: true,
    }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'PLA-CF', exact: true }).click();
  await page.getByRole('radio', { name: 'Lava Gray', exact: true }).check();
  await expect(result.getByText('Szacowana cena wydruku', { exact: true })).toBeVisible();
  await expect(result.locator('.price')).toContainText('61,00');
  await expect(page.locator('.quote-contact')).toBeEnabled();

  expect(estimates.length).toBeGreaterThan(0);
  expect(estimates.every((r) => r.material_id === 'GFG02' && r.color_code === '33102')).toBe(true);
  await page.getByRole('button', { name: 'Zapytaj o wycenę', exact: true }).click();
  await expect(page.locator('#order-title')).toBeVisible();
  const models = await page.evaluate(
    () => JSON.parse(localStorage.getItem('rapidmaker.frontend.demo.v1')!).cart.models,
  );
  const model = models[0];
  expect(models).toHaveLength(2);
  expect(models[1].analysis.id).toBe(model.analysis.id);
  expect(models[1].colorPreference).toContain('Lava Gray');
  expect(model.options.materialId).toBe('GFG02');
  expect(model.options.color).toBe('');
  expect(model.colorPreference).toContain('Matcha Green');
  await expect(page.locator('.order-total')).toContainText('Szacowana cena wydruku');
  await expect(page.locator('.order-total')).toContainText('61,00');
  await expect(
    page.getByRole('button', { name: 'Przejdź do indywidualnej wyceny', exact: true }),
  ).toBeVisible();
});

test('missing operator configuration preserves the backend reason instead of hiding it behind generic manual text', async ({
  page,
}) => {
  const estimates = await configure(page, false);
  await expect(page.locator('.quote-result')).toContainText(
    'Materiał wymaga zatwierdzenia bezpiecznego profilu i jawnej ceny zakupu.',
  );
  expect(estimates.length).toBeGreaterThan(0);
  await expect(page.locator('.quote-result .price')).toHaveCount(0);
});

test('a terminal polled failure preserves the model in a manual quote draft without a payable price', async ({
  page,
}) => {
  let automaticCheckout = 0;
  const polled = page.waitForResponse('**/api/quotes/jobs/stopped-quote');
  const estimates = await configure(
    page,
    true,
    async (route) => {
      automaticCheckout++;
      await route.fulfill({ status: 503, json: {} });
    },
    undefined,
    true,
  );
  await polled;
  await expect(page.locator('.manual-heading')).toHaveText('Wymagana ręczna wycena');
  await expect(page.locator('.quote-result')).toContainText(
    'Automatyczna wycena została zatrzymana.',
  );
  await expect(page.locator('.quote-result .price')).toHaveCount(0);
  expect(estimates.some((body) => body.model_id === modelId && body.material_id === 'GFG02')).toBe(
    true,
  );
  await page.getByRole('button', { name: 'Zapytaj o wycenę', exact: true }).click();
  await expect(page.locator('#order-title')).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Przejdź do indywidualnej wyceny', exact: true }),
  ).toBeVisible();
  const cart = await page.evaluate(
    () => JSON.parse(localStorage.getItem('rapidmaker.frontend.demo.v1')!).cart,
  );
  expect(cart.models[0].analysis.id).toBe(modelId);
  expect(cart.models[0].options.materialId).toBe('GFG02');
  expect(cart.quote).toBeNull();
  await expect(page.locator('.order-total')).toContainText('Wycena indywidualna');
  await expect(page.locator('.order-total')).toContainText('Ustalimy razem');
  expect(automaticCheckout).toBe(0);
});

// Real UI and adapters, controlled HTTP only: this is not backend pricing evidence.
test('a late previous quantity response cannot overwrite the current quote', async ({ page }) => {
  let releaseOld!: () => void;
  const held = new Promise<void>((resolve) => {
    releaseOld = resolve;
  });
  let oldStarted!: () => void;
  const started = new Promise<void>((resolve) => {
    oldStarted = resolve;
  });
  let oldFinished!: () => void;
  const finished = new Promise<void>((resolve) => {
    oldFinished = resolve;
  });
  let hold = true;
  await configure(page, true, async (route) => {
    const part = route.request().postDataJSON().parts[0];
    const old = part.quantity === 1 && hold;
    if (old) {
      oldStarted();
      await held;
    }
    const total = part.quantity === 1 ? 3650 : 7300;
    await route.fulfill({
      json: {
        id: `cart-${part.quantity}`,
        expires_at: Math.floor(Date.now() / 1000) + 600,
        packing: { status: 'estimated', parcels: [], packaging_grosze: 0 },
        offers: [],
        print_grosze: total,
        setup_grosze: 0,
        packaging_grosze: 0,
        print_lines: [
          { model_id: part.id, quantity: part.quantity, unit_grosze: 3650, total_grosze: total },
        ],
      },
    });
    if (old) oldFinished();
  });
  await started;
  // All quantity-one responses remain pending while the new quantity is priced.
  const proceed = page.getByRole('button', { name: 'Przejdź do zamówienia', exact: true });
  await expect(proceed).toBeDisabled();
  await page.locator('.model-row').getByRole('spinbutton').fill('2');
  const price = page.locator('.quote-result .price');
  await expect(price).toContainText('73,00');
  await expect(proceed).toBeEnabled();
  hold = false;
  const oldResponse = page.waitForResponse(
    async (response) =>
      response.url().endsWith('/api/checkout/quote') && (await response.json()).id === 'cart-1',
  );
  releaseOld();
  await finished;
  expect(await (await oldResponse).finished()).toBeNull();
  // Let the completed response's promise callbacks and DOM rendering settle.
  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
      ),
  );
  await expect(price).toContainText('73,00');
  await expect(page.locator('.estimate-lines')).toContainText('2 szt.');
  await expect(proceed).toBeEnabled();
  await proceed.click();
  await expect(page.locator('.order-total')).toContainText('73,00');
});

for (const reviewed of [false, true])
  test(`switching accounts drops ${reviewed ? 'reviewed' : 'inline'} contact and restores only the new account draft`, async ({
    page,
    context,
  }) => {
    let current = { id: 'inline-a', email: 'inline-a@example.test' };
    await page.addInitScript(() => {
      for (const id of ['inline-a', 'inline-b'])
        sessionStorage.setItem(
          'rapidmaker.checkout-contact.v2.' + id,
          JSON.stringify({
            contact: {
              name: id + ' private name',
              email: id + '@example.test',
              phone: '48111111111',
              street: id + ' street',
              city: 'Warszawa',
              postalCode: '00-001',
            },
            notes: id + ' private notes',
          }),
        );
    });
    await configure(page, true, undefined, () => current);
    if (reviewed) {
      await page.locator('.quote-contact').click();
      await expect(page.locator('input[name=email]')).toHaveValue('inline-a@example.test');
      // Restore a genuine model requirement through the same adapter as a cart reload.
      await page.evaluate(() => {
        const key = 'rapidmaker.frontend.demo.v1';
        const saved = JSON.parse(localStorage.getItem(key)!);
        saved.cart.models[0].notes = 'Original model requirement';
        localStorage.setItem(key, JSON.stringify(saved));
        localStorage.setItem('rapidmaker.storefront.checkout-ready', '1');
      });
      await page.goto('/wycena?restore=cart');
    }
    const open = page.locator('.quote-contact');
    await expect(open).toBeEnabled();
    await open.click();
    await expect(page.locator('input[name=email]')).toHaveValue('inline-a@example.test');
    await expect(page.locator('textarea[name=notes]')).toHaveValue('inline-a private notes');
    if (reviewed) {
      await page
        .getByRole('button', { name: 'Przejdź do indywidualnej wyceny', exact: true })
        .click();
      await expect(
        page.getByRole('heading', { name: 'Zapytaj o wycenę', exact: true }),
      ).toBeVisible();
      expect(
        await page.evaluate(() =>
          JSON.parse(localStorage.getItem('rapidmaker.frontend.demo.v1')!).cart.models.map(
            (m: { notes: string }) => m.notes,
          ),
        ),
      ).toEqual(['Original model requirement']);
      await expect(page.getByLabel('Uwagi i wymagania', { exact: true })).toHaveValue(
        'inline-a private notes',
      );
      let body: { notes: string } | undefined;
      await page.route('**/api/account/manual-quotes', async (route) => {
        body = route.request().postDataJSON();
        await route.fulfill({ status: 422, json: { message: 'Fixture captured request only.' } });
      });
      await page.getByRole('button', { name: 'Wyślij zapytanie o wycenę', exact: true }).click();
      await expect
        .poll(() => body?.notes)
        .toBe('inline-a private notes\ncube_20mm.stl: Original model requirement');
    }
    const cartBefore = await page.evaluate(
      () => JSON.parse(localStorage.getItem('rapidmaker.frontend.demo.v1')!).cart.models,
    );
    const draftBefore = await page.evaluate(() =>
      sessionStorage.getItem('rapidmaker.checkout-contact.v2.inline-a'),
    );

    // Real login UI and its session mutation publish the production auth marker.
    const other = await context.newPage();
    let signedIn = false;
    await other.route('**/api/**', async (route) => {
      const path = new URL(route.request().url()).pathname;
      if (path === '/api/auth/me')
        return route.fulfill(
          signedIn
            ? {
                json: {
                  user: { ...current, role: 'client' },
                  stage: 'authenticated',
                  csrf_token: 'fixture-csrf',
                },
              }
            : { status: 401, json: {} },
        );
      if (path === '/api/auth/password/login') {
        current = { id: 'inline-b', email: 'inline-b@example.test' };
        signedIn = true;
        return route.fulfill({
          json: { user: { ...current, role: 'client' }, stage: 'authenticated' },
        });
      }
      if (path === '/api/auth/claim-guest')
        return route.fulfill({ json: { resources_claimed: 0 } });
      if (path === '/api/account/orders') return route.fulfill({ json: { orders: [] } });
      throw new Error('Unexpected login fixture API: ' + path);
    });
    await other.goto('/logowanie');
    await other.getByLabel('Adres email', { exact: true }).fill('inline-b@example.test');
    await other.getByLabel('Hasło', { exact: true }).fill('fixture-password-only');
    await other.getByRole('button', { name: 'Zaloguj hasłem', exact: true }).click();
    await expect(other).toHaveURL(/\/konto\/zamowienia$/);
    await expect(page.locator('#order-title')).toHaveCount(0);
    expect(
      await page.evaluate(
        () => JSON.parse(localStorage.getItem('rapidmaker.frontend.demo.v1')!).cart.models,
      ),
    ).toEqual(cartBefore);
    await open.click();
    await expect(page.locator('input[name=email]')).toHaveValue('inline-b@example.test');
    await expect(page.locator('input[name=name]')).toHaveValue('inline-b private name');
    await expect(page.locator('textarea[name=notes]')).toHaveValue('inline-b private notes');
    expect(
      await page.evaluate(() => sessionStorage.getItem('rapidmaker.checkout-contact.v2.inline-a')),
    ).toBe(draftBefore);
    if (reviewed) {
      await page.goto('/zamowienie');
      await expect(page.locator('.checkout-layout, .manual-request')).toBeVisible();
      await expect(page.locator('body')).not.toContainText('inline-a private notes');
      expect(
        await page.evaluate(() =>
          JSON.stringify(JSON.parse(localStorage.getItem('rapidmaker.frontend.demo.v1')!).cart),
        ),
      ).not.toContain('inline-a private notes');
      await page.reload();
      await expect(page.locator('.checkout-layout, .manual-request')).toBeVisible();
      await expect(page.locator('body')).not.toContainText('inline-a private notes');
    }
    await other.close();
  });
