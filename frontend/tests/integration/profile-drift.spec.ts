import { expect, test } from './fixtures';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

test('full HTTP checkout rejects a stale catalog profile after a real slicer quote', async ({
  request,
}, testInfo) => {
  test.skip(
    process.env.RAPIDMAKER_SANDBOX !== '1',
    'Requires --sandbox: payment entry requires Stripe test configuration',
  );
  const database = process.env.RAPIDMAKER_TEST_DATABASE ?? '';
  expect(database).toMatch(/^rapidmaker_test_browser_[a-f0-9]{32}$/);
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
      { encoding: 'utf8', timeout: 15_000, stdio: ['ignore', 'pipe', 'pipe'] },
    ).trim();
  expect(sql('SELECT current_database()')).toBe(database);
  const headers = { Origin: 'http://localhost:18437' };
  const upload = await request.post('/api/models', {
    headers: {
      ...headers,
      'Content-Type': 'application/octet-stream',
      'x-model-filename': 'cube_20mm.stl',
    },
    data: readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl')),
  });
  expect(upload.status()).toBe(201);
  const model = await upload.json();
  const catalog = await request.get('/api/catalog/materials');
  expect(catalog.status()).toBe(200);
  const material = (await catalog.json()).materials.find(
    (item: { id: string }) => item.id === 'GFA00',
  );
  expect(material?.colors.length).toBeGreaterThan(0);
  const estimate = await request.post('/api/quotes/estimate', {
    headers,
    data: {
      model_id: model.id,
      material_id: 'GFA00',
      color_code: material.colors[0].code,
      options: { quantity: 1, quality: 'standard', wall_loops: 3, infill_percent: 20 },
    },
  });
  expect(estimate.status()).toBe(202);
  let job = await estimate.json();
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
  expect(job.result.measurement.time_seconds).toBeGreaterThan(0);
  expect(job.result.price.gross_grosze).toBeGreaterThan(0);
  const quoteId: string = job.result.id;
  expect(quoteId).toMatch(/^[a-f0-9-]{36}$/);
  const original = sql("SELECT source->>'profiles_sha256' FROM catalog_materials WHERE id='GFA00'");
  expect(original).toMatch(/^[a-f0-9]{64}$/);
  expect(sql(`SELECT request->>'profiles_sha256' FROM quotes WHERE id='${quoteId}'`)).toBe(
    original,
  );
  const checkout = () =>
    request.post('/api/checkout/quote', {
      headers,
      data: {
        parts: [{ id: model.id, analysis_id: model.id, quantity: 1, quote_id: quoteId }],
        delivery: 'pickup',
        contact: { name: 'Test Rapidmaker', email: 'void@sieciowiec.xyz', phone: '500600700' },
      },
    });
  const prepared = await checkout();
  expect(prepared.status()).toBe(200);
  const draft = await prepared.json();
  expect(draft.manual_required).toBe(false);
  expect(draft.offers).toHaveLength(1);
  expect(draft.offers[0].total_grosze).toBeGreaterThan(0);
  expect(draft.id).toMatch(/^[a-f0-9-]{36}$/);
  const legal = await request.get('/api/legal/documents');
  expect(legal.status()).toBe(200);
  const { version: legalVersion } = await legal.json();
  expect(legalVersion).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  const effects = () =>
    sql(`SELECT json_build_object(
    'payment_attempts',(SELECT count(*) FROM payment_attempts),
    'orders',(SELECT count(*) FROM orders),
    'worker_attempts',(SELECT coalesce(sum(attempts),0) FROM payment_attempts))`);
  const before = effects();
  expect(JSON.parse(before)).toEqual({ payment_attempts: 0, orders: 0, worker_attempts: 0 });
  const changed = original === 'a'.repeat(64) ? 'b'.repeat(64) : 'a'.repeat(64);
  let rejectionStatus: number;
  try {
    sql(
      `UPDATE catalog_materials SET source=jsonb_set(source,'{profiles_sha256}',to_jsonb('${changed}'::text)) WHERE id='GFA00'`,
    );
    expect(sql("SELECT source->>'profiles_sha256' FROM catalog_materials WHERE id='GFA00'")).toBe(
      changed,
    );
    const rejected = await request.post('/api/checkout/intent', {
      headers,
      data: { quote_id: draft.id, offer_id: draft.offers[0].id, legal_version: legalVersion },
    });
    rejectionStatus = rejected.status();
    expect(rejectionStatus).toBe(409);
    expect(await rejected.json()).toEqual({
      message: 'Wycena wygasła lub konfiguracja się zmieniła. Przelicz zamówienie.',
    });
    expect(effects()).toBe(before);
    expect(sql(`SELECT locked FROM checkout_drafts WHERE id='${draft.id}'`)).toBe('f');
    expect(sql(`SELECT request->>'profiles_sha256' FROM quotes WHERE id='${quoteId}'`)).toBe(
      original,
    );
  } finally {
    sql(
      `UPDATE catalog_materials SET source=jsonb_set(source,'{profiles_sha256}',to_jsonb('${original}'::text)) WHERE id='GFA00'`,
    );
  }
  // Restoring only the profile makes this same live slicer quote usable again.
  expect((await checkout()).status()).toBe(200);
  expect(effects()).toBe(before);
  await testInfo.attach('stale-profile-http-rejection', {
    body: JSON.stringify({
      rejectionStatus,
      paymentAttemptsAdded: 0,
      ordersAdded: 0,
      workerAttemptsAdded: 0,
      restoredProfileCheckoutStatus: 200,
      scope:
        'HTTP precheck rejection; does not exercise drift inside the locked storage transaction or directly observe Stripe network calls',
    }),
    contentType: 'application/json',
  });
});
