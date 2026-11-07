import { expect, test } from './fixtures';
import { resolve } from 'node:path';
import { readFileSync } from 'node:fs';
import { accounts, fillStripeCard, login, stripeEvidence } from './helpers';
import { fillAuthorContact } from './checkout-helpers';

test.use({ screenshot: 'off', trace: 'off', video: 'off' });
test('one server upload retains two configurations through Stripe and both order panels', async ({
  page,
  browser,
}) => {
  test.skip(process.env.RAPIDMAKER_SANDBOX !== '1', 'Requires real Stripe TEST sandbox');
  test.setTimeout(240_000);
  const client = accounts().find((a) => a.email === 'void@sieciowiec.xyz')!;
  test.skip(!client.totp_uri, 'Requires --client-totp');
  await login(page, client);
  type Part = { id: string; analysis_id: string; quantity: number; quote_id: string };
  type Snapshot = {
    id: string;
    measurement: { filament_grams: number; time_seconds: number; plates: number };
  };
  type Quote = {
    id: string;
    print_grosze: number;
    packaging_grosze: number;
    setup_grosze: number;
    print_lines: {
      model_id: string;
      quantity: number;
      unit_grosze: number;
      total_grosze: number;
    }[];
    offers: { id: string; price_grosze: number }[];
  };
  const jobs: Promise<{ status: string; result?: Snapshot }>[] = [];
  const quotes: { parts: Part[]; result: Promise<Quote> }[] = [];
  let uploads = 0;
  page.on('response', (r) => {
    if (r.url().endsWith('/api/models') && r.request().method() === 'POST') uploads++;
    if (
      (r.url().includes('/api/quotes/jobs/') || r.url().endsWith('/api/quotes/estimate')) &&
      r.ok()
    )
      // Reloads can evict an intermediate poll body; final referenced snapshots are required below.
      jobs.push(r.json().catch(() => ({ status: 'navigation-aborted' })));
    if (r.url().endsWith('/api/checkout/quote') && r.ok())
      quotes.push({ parts: r.request().postDataJSON().parts, result: r.json() });
  });
  await page.goto('/wycena');
  const fixture = resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl');
  await page.getByLabel('Dodaj modele STL, STEP lub 3MF').setInputFiles(fixture);
  await page.locator('input[name=material][value=pla]').check();
  await page.locator('input[name=color][value=black]').check();
  await page.locator('input[name=quality][value=standard]').check();
  await page.locator('input[name=strength][value=standard]').check();
  const proceed = page.getByRole('button', { name: 'Przejdź do zamówienia', exact: true });
  await expect(proceed).toBeEnabled({ timeout: 90_000 });
  const modelId = await page.evaluate(
    () =>
      JSON.parse(localStorage.getItem('rapidmaker.storefront.draft.v1')!).models[0].analysis
        .id as string,
  );
  await page.getByRole('button', { name: 'Powiel wariant cube_20mm.stl', exact: true }).click();
  const rows = page.locator('.model-row');
  await expect(rows).toHaveCount(2);
  await rows.nth(0).getByRole('spinbutton').fill('2');
  await rows.nth(1).getByRole('spinbutton').fill('3');
  await rows.nth(1).getByRole('button', { name: 'Konfiguruj cube_20mm.stl' }).click();
  await page.locator('input[name=color][value=white]').check();
  await page.locator('input[name=quality][value=fine]').check();
  await page.locator('input[name=strength][value=reinforced]').check();
  await expect(proceed).toBeEnabled({ timeout: 90_000 });
  await page.reload();
  await expect(rows.nth(0)).toContainText('Czarny');
  await expect(rows.nth(1)).toContainText('Biały');
  await expect(rows.nth(0).getByRole('spinbutton')).toHaveValue('2');
  await expect(rows.nth(1).getByRole('spinbutton')).toHaveValue('3');
  await expect(proceed).toBeEnabled({ timeout: 90_000 });
  await proceed.click();
  await expect(page.locator('.order-summary li')).toHaveCount(2);
  await fillAuthorContact(page);
  await page.getByRole('button', { name: 'Wybierz dostawę i zapłać', exact: true }).click();
  await page.getByRole('radio', { name: /Odbiór osobisty/ }).check();
  await page.getByRole('button', { name: 'Podsumowanie →', exact: true }).click();
  await page.getByRole('button', { name: 'Przejdź do płatności →', exact: true }).click();
  await fillStripeCard(page);
  const created = page.waitForResponse(
    (r) => r.url().endsWith('/api/checkout/intent') && r.request().method() === 'POST',
  );
  await page.getByRole('button', { name: /^Zamawiam i płacę .*→$/ }).click();
  const response = await created;
  expect(response.status()).toBe(200);
  const input = response.request().postDataJSON();
  const resolved = await Promise.all(quotes.map(async (q) => ({ ...q, quote: await q.result })));
  const { quote, parts } = resolved.find((q) => q.quote.id === input.quote_id)!;
  expect(parts).toHaveLength(2);
  expect(new Set(parts.map((p) => p.analysis_id))).toEqual(new Set([modelId]));
  expect(new Set(parts.map((p) => p.id)).size).toBe(2);
  expect(new Set(parts.map((p) => p.quote_id)).size).toBe(2);
  expect(parts.map((p) => p.quantity)).toEqual([2, 3]);
  const completed = (await Promise.all(jobs)).flatMap((j) => (j.result ? [j.result] : []));
  const costs = parts.map((p) => {
    const m = completed.find((s) => s.id === p.quote_id)!.measurement;
    // Independent disposable-fixture invoice: 100 PLN/kg, 2 PLN/h, energy, actual plates.
    return (
      Math.ceil(Math.ceil(m.filament_grams * 1000) / 100) +
      Math.ceil(m.time_seconds / 18) +
      Math.ceil(m.time_seconds / 200) +
      m.plates * 500
    );
  });
  const base = costs.reduce((sum, cost) => sum + cost, 1000); // one setup for both rows
  const net = Math.ceil(Math.max(2000, ((base + Math.ceil(base / 20)) * 10) / 7));
  const gross = net + Math.ceil((net * 23) / 100);
  expect(quote.print_grosze).toBe(gross);
  expect(quote.print_lines).toHaveLength(2);
  expect(new Set(quote.print_lines.map((line) => line.model_id))).toEqual(
    new Set(parts.map((part) => part.id)),
  );
  for (const [index, part] of parts.entries()) {
    const line = quote.print_lines.find((line) => line.model_id === part.id)!;
    expect(line.quantity).toBe(part.quantity);
    expect(Number.isSafeInteger(line.total_grosze)).toBe(true);
    expect(line.total_grosze).toBeGreaterThan(0);
    expect(line.unit_grosze).toBe(Math.ceil(line.total_grosze / part.quantity));
    expect(Math.abs(line.total_grosze - (gross * costs[index]) / base)).toBeLessThan(1);
  }
  expect(Math.abs(quote.setup_grosze - (gross * 1000) / base)).toBeLessThan(1);
  expect(quote.print_lines.reduce((sum, l) => sum + l.total_grosze, quote.setup_grosze)).toBe(
    gross,
  );
  const amount =
    gross +
    quote.packaging_grosze +
    quote.offers.find((o) => o.id === input.offer_id)!.price_grosze;
  expect((await response.json()).total_grosze).toBe(amount);
  await expect(page).toHaveURL(/\/zamowienie\?payment_intent=pi_/);
  const intent = new URL(page.url()).searchParams.get('payment_intent')!;
  const statusPath = `/api/checkout/intent/${intent}`;
  await expect
    .poll(async () => (await (await page.request.get(statusPath)).json()).status)
    .toBe('paid');
  const paid = await (await page.request.get(statusPath)).json();
  const id = paid.order_id;
  const provider = stripeEvidence({ payment_intent_id: intent, attempt_id: id, order_id: id });
  expect(provider).toMatchObject({
    status: 'succeeded',
    currency: 'pln',
    amount,
    amount_received: amount,
  });
  expect(provider.refunds).toHaveLength(0);
  expect(uploads).toBe(1);
  const operatorContext = await browser.newContext();
  try {
    const operator = await operatorContext.newPage();
    await login(
      operator,
      accounts().find((a) => a.role === 'admin')!,
    );
    for (const [target, prefix, route] of [
      [page, 'account', 'konto'],
      [operator, 'admin', 'admin'],
    ] as const) {
      const path = `/api/${prefix}/orders/${id}`;
      const orderResponse = await target.request.get(
        prefix === 'admin' ? '/api/admin/orders' : path,
      );
      expect(orderResponse.status()).toBe(200);
      const body = await orderResponse.json();
      const order =
        prefix === 'admin' ? body.orders.find((o: { id: string }) => o.id === id) : body;
      expect(order).toMatchObject({ amount_grosze: amount, payment_state: 'paid' });
      expect(order.lines).toHaveLength(2);
      expect(new Set(order.lines.map((l: { line_id: string }) => l.line_id))).toEqual(
        new Set(parts.map((p) => p.id)),
      );
      const filesResponse = await target.request.get(path + '/files');
      expect(filesResponse.status()).toBe(200);
      const { files } = await filesResponse.json();
      expect(files).toHaveLength(2);
      for (const [index, part] of parts.entries()) {
        expect(order.lines.find((l: { line_id: string }) => l.line_id === part.id)).toMatchObject({
          model_id: modelId,
          quote_id: part.quote_id,
          quantity: part.quantity,
          total_grosze: quote.print_lines.find((line) => line.model_id === part.id)!.total_grosze,
        });
        const file = files.find((f: { quote_id: string }) => f.quote_id === part.quote_id);
        expect(file).toMatchObject({
          model_id: modelId,
          quantity: part.quantity,
          available: true,
          color_code: index === 0 ? '10101' : '10100',
          options: {
            quality: index === 0 ? 'standard' : 'fine',
            wall_loops: index === 0 ? 3 : 5,
            infill_percent: index === 0 ? 20 : 40,
          },
        });
        const source = await target.request.get(file.source_url);
        expect(source.status()).toBe(200);
        expect(await source.body()).toEqual(readFileSync(fixture));
        expect((await target.request.get(file.preview_url)).status()).toBe(200);
      }
      await target.goto(`/${route}/zamowienia/${id}`);
      for (let reload = 0; reload < 2; reload++) {
        const models = target.locator('article.model');
        await expect(models).toHaveCount(2);
        for (const [index, quantity] of [2, 3].entries()) {
          await expect(models.nth(index)).toContainText(`${quantity} szt.`);
          await expect(models.nth(index)).toContainText(
            files.find((f: { quote_id: string }) => f.quote_id === parts[index].quote_id)
              .color_name,
          );
          await models.nth(index).getByText('Parametry wydruku', { exact: true }).click();
          await expect(models.nth(index)).toContainText(index === 0 ? 'standardowa' : 'dokładna');
          await expect(models.nth(index)).toContainText(
            index === 0 ? 'Ściany: 3; wypełnienie: 20%' : 'Ściany: 5; wypełnienie: 40%',
          );
        }
        await expect(
          target.getByRole('link', { name: 'Pobierz oryginał ↗', exact: true }),
        ).toHaveCount(2);
        if (!reload) await target.reload();
      }
    }
  } finally {
    await operatorContext.close();
  }
  await test.info().attach('same-upload-configs-evidence', {
    contentType: 'application/json',
    body: JSON.stringify({
      uploads,
      model_id: modelId,
      parts,
      order_id: id,
      measured_costs_grosze: costs,
      print_grosze: gross,
      amount_received_grosze: provider.amount_received,
      currency: provider.currency,
      panels: ['customer', 'admin'],
    }),
  });
});
