import { expect, test } from './fixtures';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

// pricing.spec covers standard 3/20 at 1/10/1000 copies; configurator covers local
// selection, and same-upload-configs covers standard/fine checkout. Exercise the
// missing draft/boundary profiles and exact PETG/TPU invoices with explicit fixture costs.
test('native options boundaries retain distinct snapshots and exact measured invoices', async ({
  request,
}) => {
  test.setTimeout(480_000);
  const headers = { Origin: 'http://localhost:18437' };
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
      { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
    ).trim();
  const source = readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
  const upload = await request.post('/api/models', {
    headers: {
      ...headers,
      'Content-Type': 'application/octet-stream',
      'x-model-filename': 'cube_20mm.stl',
    },
    data: source,
  });
  expect(upload.status()).toBe(201);
  const model = await upload.json();
  expect(model.id).toMatch(/^[a-f0-9-]{36}$/);
  const catalog = await (await request.get('/api/catalog/materials')).json();
  const estimate = (options: object, material_id = 'GFA00') => {
    const material = catalog.materials.find((m: { id: string }) => m.id === material_id);
    return request.post('/api/quotes/estimate', {
      headers,
      data: {
        model_id: model.id,
        material_id: material.id,
        color_code: material.colors[0].code,
        options,
      },
    });
  };
  const count = () => Number(sql(`SELECT count(*) FROM slicer_jobs WHERE model_id='${model.id}'`));
  const normal = { quantity: 1, quality: 'standard', wall_loops: 3, infill_percent: 20 };
  const invalid = [
    { quantity: 0 },
    { quantity: 1001 },
    { wall_loops: 1 },
    { wall_loops: 9 },
    { infill_percent: 4 },
    { infill_percent: 101 },
    { quality: 'ultra' },
    { wall_loops: 2.5 },
    { layer_height: 0.01 },
  ];
  expect(count()).toBe(0);
  for (const patch of invalid) {
    const response = await estimate({ ...normal, ...patch });
    expect(response.status(), JSON.stringify(patch)).toBe(
      'quality' in patch || 'layer_height' in patch || patch.wall_loops === 2.5 ? 422 : 400,
    );
    expect(count(), 'Invalid options must not create a queued or failed job').toBe(0);
  }

  const cases: (typeof normal & { profile: string; material_id?: string; cost?: number })[] = [
    {
      quantity: 1,
      quality: 'standard',
      wall_loops: 2,
      infill_percent: 5,
      profile: '0.20mm Standard',
    },
    {
      quantity: 2,
      quality: 'draft',
      wall_loops: 8,
      infill_percent: 5,
      profile: '0.28mm Extra Draft',
    },
    { quantity: 3, quality: 'fine', wall_loops: 2, infill_percent: 100, profile: '0.12mm Fine' },
    { ...normal, material_id: 'GFG02', cost: 10000, profile: '0.20mm Standard' },
    { ...normal, material_id: 'GFU01', cost: 20000, profile: '0.20mm Standard' },
  ];
  const evidence: object[] = [];
  const ids = new Set<string>();
  const measurements = test.info().outputPath('measurements.json');
  const ceil = (numerator: number, denominator: number) =>
    Number((BigInt(numerator) + BigInt(denominator) - 1n) / BigInt(denominator));
  // This mutation is restricted by the disposable database name guard above.
  const tpuCost = sql("SELECT cost_net_grosze_per_kg FROM catalog_materials WHERE id='GFU01'");
  expect(tpuCost).toBe('10000');
  expect(sql("SELECT production_enabled FROM catalog_materials WHERE id='GFG02'")).toBe('t');
  try {
    sql("UPDATE catalog_materials SET production_enabled=false WHERE id='GFG02'");
    const blocked = await estimate(normal, 'GFG02');
    expect(blocked.status()).toBe(422);
    expect(await blocked.json()).toMatchObject({
      code: 'manual_quote_required',
      reason: 'profile',
    });
    expect(count(), 'Revoked approval must not create a slicer job').toBe(0);
  } finally {
    sql("UPDATE catalog_materials SET production_enabled=true WHERE id='GFG02'");
    expect(sql("SELECT production_enabled FROM catalog_materials WHERE id='GFG02'")).toBe('t');
  }
  try {
    sql("UPDATE catalog_materials SET cost_net_grosze_per_kg=20000 WHERE id='GFU01'");
    for (const { profile, material_id = 'GFA00', cost = 10000, ...options } of cases) {
      expect(
        Number(
          sql(`SELECT cost_net_grosze_per_kg FROM catalog_materials WHERE id='${material_id}'`),
        ),
      ).toBe(cost);
      const response = await estimate(options, material_id);
      expect(response.status()).toBe(202);
      let job = await response.json();
      expect(ids.has(job.id), 'Changed settings must not reuse another configuration').toBe(false);
      await expect
        .poll(
          async () => {
            const poll = await request.get(job.poll_url);
            expect(poll.status()).toBe(200);
            job = await poll.json();
            return job.status;
          },
          { timeout: 100_000, intervals: [1000, 2000] },
        )
        .toMatch(/^(succeeded|failed)$/);
      expect(job.status, job.message ?? JSON.stringify(options)).toBe('succeeded');
      const result = job.result;
      expect(result).toMatchObject({
        id: job.id,
        model_id: model.id,
        quantity: options.quantity,
        options,
        currency: 'PLN',
      });
      expect(result.id).toMatch(/^[a-f0-9-]{36}$/);
      ids.add(result.id);
      const { measurement: m } = result;
      expect(m.profile_version).toContain(profile);
      expect(m.profile_version).toContain(
        `walls${options.wall_loops}-infill${options.infill_percent}`,
      );
      expect(m.quantity).toBe(options.quantity);
      expect(m.plates).toBe(result.plates);
      type Batch = {
        copies_per_plate: number;
        repeat_count: number;
        time_seconds: number;
        filament_grams: number;
      };
      const batches: Batch[] = m.batches;
      expect(batches.length).toBeGreaterThan(0);
      for (const b of batches) {
        for (const n of [b.copies_per_plate, b.repeat_count, b.time_seconds]) {
          expect(Number.isSafeInteger(n)).toBe(true);
          expect(n).toBeGreaterThan(0);
        }
        expect(b.filament_grams).toBeGreaterThan(0);
      }
      expect(batches.reduce((n, b) => n + b.copies_per_plate * b.repeat_count, 0)).toBe(
        options.quantity,
      );
      expect(batches.reduce((n, b) => n + b.repeat_count, 0)).toBe(m.plates);
      expect(batches.reduce((n, b) => n + b.time_seconds * b.repeat_count, 0)).toBe(m.time_seconds);
      expect(batches.reduce((n, b) => n + b.filament_grams * b.repeat_count, 0)).toBeCloseTo(
        m.filament_grams,
        8,
      );
      // Independently invoice measured totals at the disposable runner's explicit rates.
      const filament_grosze = ceil(Math.ceil(m.filament_grams * 1000) * cost, 1000000);
      const machine_grosze = ceil(m.time_seconds * 200, 3600);
      const energy_grosze = ceil(m.time_seconds * 150 * 120, 3600000);
      const handling_grosze = 500 * m.plates;
      const base = filament_grosze + machine_grosze + energy_grosze + handling_grosze + 1000;
      const loss_grosze = ceil(base * 500, 10000);
      const cost_net_grosze = base + loss_grosze;
      const net_grosze = Math.max(2000, ceil(cost_net_grosze * 10000, 7000));
      const vat_grosze = ceil(net_grosze * 2300, 10000);
      const expected = {
        filament_grosze,
        machine_grosze,
        energy_grosze,
        handling_grosze,
        setup_grosze: 1000,
        loss_grosze,
        cost_net_grosze,
        net_grosze,
        vat_grosze,
        gross_grosze: net_grosze + vat_grosze,
      };
      expect(result.price).toEqual(expected);
      const saved = JSON.parse(
        sql(
          `SELECT jsonb_build_object('request',request,'result',result) FROM quotes WHERE id='${result.id}'`,
        ),
      );
      expect(saved.request).toMatchObject({
        model_id: model.id,
        options,
        quantity: options.quantity,
        material_id,
        purchase_net_grosze_per_kg: cost,
        rates: { version: 'browser-test-only' },
      });
      expect(saved.request.slicer_runtime_sha256).toMatch(/^[a-f0-9]{64}$/);
      expect(saved.result).toEqual(result);
      // Returning to an unchanged configuration reuses its still-valid measured snapshot.
      const repeated = await estimate(options, material_id);
      expect(repeated.status()).toBe(202);
      expect(await repeated.json()).toMatchObject({ id: result.id, status: 'succeeded', result });
      expect(count()).toBe(ids.size);
      evidence.push({
        material_id,
        purchase_net_grosze_per_kg: cost,
        options,
        result,
        expected_price: expected,
        runtime_sha256: saved.request.slicer_runtime_sha256,
      });
      writeFileSync(
        measurements,
        JSON.stringify(
          {
            source_sha256: createHash('sha256').update(source).digest('hex'),
            invalid_options_rejected_without_jobs: invalid,
            revoked_approval_rejected_without_jobs: true,
            cases: evidence,
          },
          null,
          2,
        ) + '\n',
        { mode: 0o600 },
      );
    }
    expect(ids.size).toBe(5);
  } finally {
    sql(`UPDATE catalog_materials SET cost_net_grosze_per_kg=${tpuCost} WHERE id='GFU01'`);
    expect(sql("SELECT cost_net_grosze_per_kg FROM catalog_materials WHERE id='GFU01'")).toBe(
      tpuCost,
    );
    expect(sql("SELECT production_enabled FROM catalog_materials WHERE id='GFG02'")).toBe('t');
    test.info().annotations.push({
      type: 'fixture-restoration',
      description: 'TPU cost and PETG approval restored',
    });
  }
  await test.info().attach('native-options-measurements', {
    path: measurements,
    contentType: 'application/json',
  });
});
