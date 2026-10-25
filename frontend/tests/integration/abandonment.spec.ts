import { expect, test } from '@playwright/test';
import { readFileSync, existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';
import { randomUUID } from 'node:crypto';

test.use({ screenshot: 'off', trace: 'off' });
test('unpaid Stripe test session is closed before its expired upload is deleted', async ({
  request,
}) => {
  test.skip(
    process.env.RAPIDMAKER_SANDBOX !== '1',
    'Requires --sandbox; creates and expires an unpaid test session',
  );
  test.setTimeout(120_000);
  const headers = { Origin: 'http://localhost:18437' };
  const catalog = await (await request.get('/api/catalog/materials')).json();
  const upload = async () =>
    request.post('/api/models', {
      headers: { ...headers, 'x-model-filename': 'cube_20mm.stl' },
      data: readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl')),
    });
  const model = await (await upload()).json();
  const estimate = await request.post('/api/quotes/estimate', {
    headers,
    data: {
      model_id: model.id,
      material_id: 'GFA00',
      color_code: catalog.materials[0].colors[0].code,
      options: { quantity: 1, quality: 'standard', wall_loops: 3, infill_percent: 20 },
    },
  });
  expect(estimate.status()).toBe(202);
  const job = await estimate.json();
  let result: any;
  await expect
    .poll(
      async () => {
        result = await (await request.get(job.poll_url)).json();
        return result.status;
      },
      { timeout: 90_000, intervals: [300, 500, 1000] },
    )
    .toBe('succeeded');
  const created = await request.post('/api/checkout/drafts', {
    headers,
    data: {
      parts: [{ id: randomUUID(), quantity: 1, analysis_id: model.id, quote_id: result.result.id }],
      recipient: { name: 'Test Rapidmaker', email: 'void@sieciowiec.xyz', phone: '500600700' },
      delivery: { method: 'pickup' },
      email_order_updates: false,
    },
  });
  expect(created.status()).toBe(201);
  const draft = await created.json();
  const priced = await request.post(`/api/checkout/drafts/${draft.id}/quote`, {
    headers,
    data: { version: draft.version },
  });
  expect(priced.status()).toBe(200);
  const quote = await priced.json();
  const started = await request.post(`/api/checkout/drafts/${draft.id}/payment`, {
    headers: { ...headers, 'Idempotency-Key': randomUUID() },
    data: { version: quote.version, offer_id: quote.quote.offers[0].id },
  });
  expect(started.status()).toBe(200);
  const attempt = await started.json();
  expect(attempt.state).toBe('open');
  expect(attempt.order_id).toMatch(/^[a-f0-9-]{36}$/);
  const database = process.env.RAPIDMAKER_TEST_DATABASE!,
    root = process.env.RAPIDMAKER_TEST_MODEL_ROOT!;
  expect(database).toMatch(/^rapidmaker_test_browser_[a-f0-9]+$/);
  expect(root).toMatch(/^\/tmp\/rapidmaker-browser-[^/]+\/models$/);
  const sql = (statement: string) =>
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
        statement,
      ],
      { stdio: 'pipe' },
    );
  sql(
    `UPDATE private_models SET expires_at=extract(epoch from now())::bigint-1 WHERE id='${model.id}'`,
  );
  // An open payment holds files even if the original 24-hour upload has expired.
  expect((await upload()).status()).toBe(201);
  expect(existsSync(resolve(root, model.id, 'preview.glb'))).toBe(true);
  sql(
    `UPDATE payment_attempts SET created_at=extract(epoch from now())::bigint-86401 WHERE id='${attempt.order_id}'`,
  );
  const status = await request.post(`/api/account/orders/${attempt.order_id}/payment-status`, {
    headers,
    data: {},
  });
  expect(status.status()).toBe(200);
  await expect
    .poll(
      async () =>
        (await (await request.get(`/api/account/orders/${attempt.order_id}`)).json()).payment_state,
    )
    .toBe('expired');
  expect((await upload()).status()).toBe(201);
  expect(existsSync(resolve(root, model.id))).toBe(false);
  expect((await request.get(model.preview_url)).status()).toBe(404);
});
