import { expect, test } from './fixtures';
import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';

const fixture = (name: string) => resolve('../backend/crates/geometry/tests/fixtures', name);

test('real configurator prices stock=false PLA with the slicer and invalidates quantity', async ({
  page,
}) => {
  await page.goto('/wycena', { waitUntil: 'domcontentloaded' });
  await page.getByLabel('Dodaj modele STL, STEP lub 3MF').setInputFiles(fixture('cube_20mm.stl'));
  await expect(page.getByRole('img', { name: /Model z pliku cube_20mm.stl/ })).toBeVisible();
  await page.locator('input[name=material][value=pla]').check();
  await page.locator('input[name=color]').first().check();
  await page.locator('input[name=quality][value=standard]').check();
  const quoteSent = page.waitForResponse(
    (r) => r.url().endsWith('/api/quotes/estimate') && r.request().method() === 'POST',
  );
  await page.locator('input[name=strength][value=standard]').check();
  expect((await quoteSent).status()).toBe(202);
  await expect(page.locator('.price')).toContainText(/\d.*zł/, { timeout: 90_000 });
  await expect(
    page.getByRole('button', { name: 'Przejdź do zamówienia', exact: true }),
  ).toBeEnabled();
  const previous = await page.locator('.price').innerText();
  await page.getByRole('spinbutton', { name: 'Liczba sztuk: cube_20mm.stl' }).fill('10');
  await expect(
    page.getByRole('button', { name: 'Przejdź do zamówienia', exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole('button', { name: 'Przejdź do zamówienia', exact: true }),
  ).toBeEnabled({
    timeout: 90_000,
  });
  expect(await page.locator('.price').innerText()).not.toBe(previous);
  await page.screenshot({
    path: 'tests/artifacts/integration/priced-configurator.png',
    fullPage: true,
  });
});

test('real batch prices follow measured plates and fixture costs for 1, 10, 26 and 1000 copies', async ({
  request,
}) => {
  test.setTimeout(600_000);
  const headers = { Origin: 'http://localhost:18437' };
  const upload = await request.post('/api/models', {
    headers: {
      ...headers,
      'Content-Type': 'application/octet-stream',
      'x-model-filename': 'cube_20mm.stl',
    },
    data: readFileSync(fixture('cube_20mm.stl')),
  });
  expect(upload.status()).toBe(201);
  const model = await upload.json();
  const catalogResponse = await request.get('/api/catalog/materials');
  expect(catalogResponse.status()).toBe(200);
  const catalog = await catalogResponse.json();
  const material = catalog.materials.find((m: { id: string }) => m.id === 'GFA00');
  expect(material?.colors.length).toBeGreaterThan(0);
  const totals: { quantity: number; gross: number }[] = [];
  for (const quantity of [1, 10, 26, 1000]) {
    const options = { quantity, quality: 'standard', wall_loops: 3, infill_percent: 20 };
    const quote = await request.post('/api/quotes/estimate', {
      headers,
      data: {
        model_id: model.id,
        material_id: material.id,
        color_code: material.colors[0].code,
        options,
      },
    });
    expect(quote.status()).toBe(202);
    let job = await quote.json();
    await expect
      .poll(
        async () => {
          const response = await request.get(job.poll_url);
          expect(response.status()).toBe(200);
          job = await response.json();
          return job.status;
        },
        { timeout: 180_000, intervals: [1000, 2000] },
      )
      .toMatch(/^(succeeded|failed)$/);
    expect(job.status, job.message ?? `Slicer batch quantity=${quantity}`).toBe('succeeded');
    if (quantity === 1) {
      const database = process.env.RAPIDMAKER_TEST_DATABASE ?? '';
      expect(database).toMatch(/^rapidmaker_test_browser_[a-f0-9]+$/);
      expect(job.result.id).toMatch(/^[a-f0-9-]{36}$/);
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
            '-At',
            '-v',
            'ON_ERROR_STOP=1',
            '-c',
            query,
          ],
          { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
        ).trim();
      const id = job.result.id;
      const digest = sql(`SELECT request->>'slicer_runtime_sha256' FROM quotes WHERE id='${id}'`);
      expect(digest).toMatch(/^[a-f0-9]{64}$/);
      const names = JSON.parse(
        sql(
          `SELECT jsonb_build_array(q.request->>'material_name',q.request->>'color_name',m.name,c.name) FROM quotes q JOIN catalog_materials m ON m.id=q.request->>'material_id' JOIN catalog_colors c ON c.material_id=m.id AND c.code=q.request->>'color_code' WHERE q.id='${id}'`,
        ),
      );
      expect(names.slice(0, 2)).toEqual(names.slice(2));
      expect(names.every((name: unknown) => typeof name === 'string' && name.length > 0)).toBe(
        true,
      );
      const checkout = () =>
        request.post('/api/checkout/quote', {
          headers,
          data: {
            parts: [{ id: model.id, analysis_id: model.id, quantity, quote_id: id }],
            delivery: 'pickup',
          },
        });
      const fresh = await checkout();
      expect(fresh.status()).toBe(200);
      expect((await fresh.json()).print_grosze).toBeGreaterThan(0);
      // Only alter the disposable test database, never the installed slicer or developer data.
      for (const expression of [
        "request - 'slicer_runtime_sha256'",
        `jsonb_set(request, '{slicer_runtime_sha256}', '"${'0'.repeat(64)}"')`,
      ]) {
        sql(`UPDATE quotes SET request=${expression} WHERE id='${id}'`);
        expect((await checkout()).status()).toBe(409);
      }
      sql(
        `UPDATE quotes SET request=jsonb_set(request, '{slicer_runtime_sha256}', '"${digest}"') WHERE id='${id}'`,
      );
      expect((await checkout()).status()).toBe(200);
    }
    const { measurement, price, plates } = job.result;
    expect(job.result).toMatchObject({ quantity, currency: 'PLN', options, model_id: model.id });
    expect(measurement.quantity).toBe(quantity);
    expect(measurement.plates).toBe(plates);
    type Batch = {
      copies_per_plate: number;
      repeat_count: number;
      time_seconds: number;
      filament_grams: number;
    };
    const batches: Batch[] = measurement.batches;
    expect(batches.length).toBeGreaterThan(0);
    if (quantity === 26) {
      // This controlled cube/profile fits 25 copies: measure the last copy separately.
      expect(plates).toBe(2);
      expect(
        batches
          .map(({ copies_per_plate, repeat_count }) => ({ copies_per_plate, repeat_count }))
          .sort((a, b) => a.copies_per_plate - b.copies_per_plate),
      ).toEqual([
        { copies_per_plate: 1, repeat_count: 1 },
        { copies_per_plate: 25, repeat_count: 1 },
      ]);
    }
    for (const batch of batches) {
      for (const value of [batch.copies_per_plate, batch.repeat_count, batch.time_seconds]) {
        expect(Number.isSafeInteger(value)).toBe(true);
        expect(value).toBeGreaterThan(0);
      }
      expect(batch.filament_grams).toBeGreaterThan(0);
    }
    expect(batches.reduce((n, b) => n + b.copies_per_plate * b.repeat_count, 0)).toBe(quantity);
    expect(batches.reduce((n, b) => n + b.repeat_count, 0)).toBe(plates);
    expect(batches.reduce((n, b) => n + b.time_seconds * b.repeat_count, 0)).toBe(
      measurement.time_seconds,
    );
    expect(batches.reduce((n, b) => n + b.filament_grams * b.repeat_count, 0)).toBeCloseTo(
      measurement.filament_grams,
      8,
    );
    for (const amount of Object.values(price)) {
      expect(Number.isSafeInteger(amount)).toBe(true);
      expect(amount).toBeGreaterThan(0);
    }
    // Disposable fixture: 10000 net grosze/kg; API rounds grams up to milligrams first.
    expect(price.filament_grosze).toBe(
      Math.ceil(Math.ceil(measurement.filament_grams * 1000) / 100),
    );
    expect(price.handling_grosze).toBe(500 * plates);
    expect(price.setup_grosze).toBe(1000);
    // Check physical units and invoice intervals independently of the Rust calculator:
    // each integer amount must round upward by less than one grosz.
    const roundedInvoiceLine = (actual: number, exact: number) => {
      expect(actual - exact).toBeGreaterThanOrEqual(-1e-8);
      expect(actual - exact).toBeLessThan(1 - 1e-8);
    };
    const hours = measurement.time_seconds / 3600;
    roundedInvoiceLine(price.machine_grosze, hours * 200);
    roundedInvoiceLine(price.energy_grosze, hours * 0.15 * 120);
    const base =
      price.filament_grosze + price.machine_grosze + price.energy_grosze + 500 * plates + 1000;
    roundedInvoiceLine(price.loss_grosze, base * 0.05);
    expect(price.cost_net_grosze).toBe(base + price.loss_grosze);
    roundedInvoiceLine(price.net_grosze, Math.max(2000, price.cost_net_grosze / 0.7));
    roundedInvoiceLine(price.vat_grosze, price.net_grosze * 0.23);
    expect(price.gross_grosze).toBe(price.net_grosze + price.vat_grosze);
    const previous = totals.at(-1);
    if (previous) {
      expect(price.gross_grosze).toBeGreaterThan(previous.gross);
      expect(price.gross_grosze / quantity).toBeLessThan(previous.gross / previous.quantity);
    }
    totals.push({ quantity, gross: price.gross_grosze });
    test.info().annotations.push({
      type: 'batch-price',
      description: JSON.stringify({
        quantity,
        plates,
        batches,
        grams: measurement.filament_grams,
        seconds: measurement.time_seconds,
        price,
      }),
    });
  }
});

for (const minimum of [2000, 50000]) {
  test(`real multi-model checkout combines measured costs once (minimum ${minimum}) and rejects altered or foreign quotes`, async ({
    request,
    playwright,
    baseURL,
  }) => {
    test.setTimeout(240_000);
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
          '-At',
          '-v',
          'ON_ERROR_STOP=1',
          '-c',
          query,
        ],
        { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], timeout: 15000 },
      ).trim();
    const original = sql(
      "SELECT encode(convert_to(value::text,'UTF8'),'hex') FROM pricing_config WHERE id=true",
    );
    expect(original).toMatch(/^(?:[a-f0-9]{2})*$/);
    // Explicit test rates, before any real slicer snapshots; restore even after assertion failure.
    const rates = {
      version: `browser-test-minimum-${minimum}`,
      setup_grosze: 1000,
      handling_per_plate_grosze: 500,
      machine_per_hour_grosze: 200,
      energy_per_kwh_grosze: 120,
      printer_watts: 150,
      loss_bps: 500,
      margin_bps: 3000,
      vat_bps: 2300,
      minimum_net_grosze: minimum,
    };
    try {
      sql(
        `INSERT INTO pricing_config(id,value) VALUES(true,'${JSON.stringify(rates)}'::jsonb) ON CONFLICT(id) DO UPDATE SET value=EXCLUDED.value`,
      );
      const headers = { Origin: 'http://localhost:18437' };
      const upload = (client: typeof request) =>
        client.post('/api/models', {
          headers: {
            ...headers,
            'Content-Type': 'application/octet-stream',
            'x-model-filename': 'cube_20mm.stl',
          },
          data: readFileSync(fixture('cube_20mm.stl')),
        });
      const catalogResponse = await request.get('/api/catalog/materials');
      expect(catalogResponse.status()).toBe(200);
      const material = (await catalogResponse.json()).materials.find(
        (m: { id: string }) => m.id === 'GFA00',
      );
      expect(material?.colors.length).toBeGreaterThan(0);
      const parts: { id: string; analysis_id: string; quantity: number; quote_id: string }[] = [];
      const measuredCosts: number[] = [];
      let separateGross = 0;
      for (const quantity of [1, 10]) {
        // Identical geometry, separate real uploads: ownership and snapshot binding use model IDs.
        const uploaded = await upload(request);
        expect(uploaded.status()).toBe(201);
        const model = await uploaded.json();
        const quote = await request.post('/api/quotes/estimate', {
          headers,
          data: {
            model_id: model.id,
            material_id: material.id,
            color_code: material.colors[0].code,
            options: { quantity, quality: 'standard', wall_loops: 3, infill_percent: 20 },
          },
        });
        expect(quote.status()).toBe(202);
        let job = await quote.json();
        await expect
          .poll(
            async () => {
              const response = await request.get(job.poll_url);
              expect(response.status()).toBe(200);
              job = await response.json();
              return job.status;
            },
            { timeout: 90_000, intervals: [1000, 2000] },
          )
          .toMatch(/^(succeeded|failed)$/);
        expect(job.status, job.message).toBe('succeeded');
        const { measurement, price } = job.result;
        expect(job.result).toMatchObject({ model_id: model.id, quantity, currency: 'PLN' });
        expect(measurement.quantity).toBe(quantity);
        expect(measurement.filament_grams).toBeGreaterThan(0);
        expect(measurement.time_seconds).toBeGreaterThan(0);
        expect(measurement.plates).toBeGreaterThan(0);
        // Invoice units from the disposable runner's fixture: 100 PLN/kg, 2 PLN/h,
        // 150 W at 1.20 PLN/kWh, 5 PLN per actual plate. Round each measured line up.
        const components = {
          filament_grosze: Math.ceil(Math.ceil(measurement.filament_grams * 1000) / 100),
          machine_grosze: Math.ceil(measurement.time_seconds / 18),
          energy_grosze: Math.ceil(measurement.time_seconds / 200),
          handling_grosze: measurement.plates * 500,
        };
        expect(price).toMatchObject(components);
        measuredCosts.push(Object.values(components).reduce((sum, cost) => sum + cost, 0));
        separateGross += price.gross_grosze;
        test.info().annotations.push({
          type: 'measured-cart-part',
          description: JSON.stringify({
            minimum,
            quantity,
            model_id: model.id,
            quote_id: job.result.id,
            measurement,
            components,
            price,
          }),
        });
        parts.push({ id: model.id, analysis_id: model.id, quantity, quote_id: job.result.id });
      }
      expect(parts[0].analysis_id).not.toBe(parts[1].analysis_id);
      expect(parts[0].quote_id).not.toBe(parts[1].quote_id);
      const checkout = (items = parts, client = request) =>
        client.post('/api/checkout/quote', {
          headers,
          data: {
            parts: items,
            delivery: 'pickup',
            contact: {
              name: 'Test Rapidmaker',
              email: 'void@sieciowiec.xyz',
              phone: '500600700',
            },
          },
        });
      const response = await checkout();
      expect(response.status()).toBe(200);
      const order = await response.json();
      // One order invoice: setup 10 PLN, 5% loss, 30% margin, test minimum,
      // then 23% VAT. Never sum the individually marked-up/minimum-priced quotes.
      const base = measuredCosts.reduce((sum, cost) => sum + cost, 1000);
      const costNet = base + Math.ceil(base / 20);
      const net = Math.ceil(Math.max(minimum, (costNet * 10) / 7));
      const gross = net + Math.ceil((net * 23) / 100);
      if (minimum === 50000) {
        expect(costNet / 0.7).toBeLessThan(minimum);
        expect(gross).toBe(61500);
        expect(separateGross).toBe(2 * gross);
      } else {
        expect(costNet / 0.7).toBeGreaterThan(minimum);
      }
      expect(order.print_grosze).toBe(gross);
      expect(order.print_grosze).toBeLessThan(separateGross);
      expect(order.manual_required).toBe(false);
      expect(order.packaging_grosze).toBe(0);
      expect(order.offers).toHaveLength(1);
      expect(order.offers[0]).toMatchObject({ price_grosze: 0, total_grosze: gross });
      expect(order.print_lines).toHaveLength(2);
      type Line = { model_id: string; quantity: number; total_grosze: number; unit_grosze: number };
      const lines: Line[] = order.print_lines;
      for (const [index, part] of parts.entries()) {
        const line = lines.find((item) => item.model_id === part.id);
        expect(line).toBeDefined();
        expect(line!.quantity).toBe(part.quantity);
        expect(Number.isSafeInteger(line!.total_grosze)).toBe(true);
        expect(line!.total_grosze).toBeGreaterThan(0);
        expect(line!.unit_grosze).toBe(Math.ceil(line!.total_grosze / part.quantity));
        // Gross allocation must reflect physical costs, allowing only grosz rounding.
        expect(Math.abs(line!.total_grosze - (gross * measuredCosts[index]) / base)).toBeLessThan(
          1,
        );
      }
      expect(Math.abs(order.setup_grosze - (gross * 1000) / base)).toBeLessThan(1);
      expect(lines.reduce((sum, line) => sum + line.total_grosze, order.setup_grosze)).toBe(gross);

      const reversedResponse = await checkout([...parts].reverse());
      expect(reversedResponse.status()).toBe(200);
      const reversed = await reversedResponse.json();
      expect(reversed.print_grosze).toBe(gross);
      expect(
        (reversed.print_lines as Line[]).reduce(
          (sum, line) => sum + line.total_grosze,
          reversed.setup_grosze,
        ),
      ).toBe(gross);
      test.info().annotations.push({
        type: 'cart-invoice',
        description: JSON.stringify({
          minimum,
          measuredCosts,
          base,
          costNet,
          net,
          gross,
          separateGross,
          setup: order.setup_grosze,
          lines,
          reversedSetup: reversed.setup_grosze,
          reversedLines: reversed.print_lines,
        }),
      });

      expect((await checkout([{ ...parts[0], quantity: 2 }, parts[1]])).status()).toBe(409);
      expect(
        (await checkout([{ ...parts[0], analysis_id: parts[1].analysis_id }, parts[1]])).status(),
      ).toBe(409);
      expect((await checkout([parts[0], { ...parts[1], id: parts[0].id }])).status()).toBe(400);
      const foreign = await playwright.request.newContext({ baseURL });
      try {
        // Give the other guest a valid identity, then try this owner's real quote snapshots.
        expect((await upload(foreign)).status()).toBe(201);
        expect((await checkout(parts, foreign)).status()).toBe(409);
      } finally {
        await foreign.dispose();
      }
      expect((await checkout()).status()).toBe(200);
    } finally {
      sql(
        original
          ? `UPDATE pricing_config SET value=convert_from(decode('${original}','hex'),'UTF8')::jsonb WHERE id=true`
          : 'DELETE FROM pricing_config WHERE id=true',
      );
      expect(
        sql(
          "SELECT encode(convert_to(value::text,'UTF8'),'hex') FROM pricing_config WHERE id=true",
        ),
      ).toBe(original);
    }
  });
}

test('oversized server geometry requests manual handling before any slicer job', async ({
  request,
}) => {
  const upload = await request.post('/api/models', {
    headers: {
      'Content-Type': 'application/octet-stream',
      Origin: 'http://localhost:18437',
      'x-model-filename': 'huge_cube_1000mm.stl',
    },
    data: readFileSync(fixture('huge_cube_1000mm.stl')),
  });
  expect(upload.status()).toBe(201);
  const model = await upload.json();
  const catalog = await (await request.get('/api/catalog/materials')).json();
  const quote = await request.post('/api/quotes/estimate', {
    headers: { Origin: 'http://localhost:18437' },
    data: {
      model_id: model.id,
      material_id: 'GFA00',
      color_code: catalog.materials[0].colors[0].code,
      options: { quantity: 1, quality: 'standard', wall_loops: 3, infill_percent: 20 },
    },
  });
  expect(quote.status()).toBe(422);
  const error = await quote.json();
  expect(error).toMatchObject({ code: 'manual_quote_required', reason: 'size_limit' });
});
