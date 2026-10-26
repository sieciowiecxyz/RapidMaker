import { test, expect } from './fixtures';
import { accounts, login } from './helpers';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { randomUUID } from 'node:crypto';

test.use({ screenshot: 'off', trace: 'off' });
test('five storefront families have real profiles; requested swatches remain manual and owned', async ({
  page,
  browser,
}) => {
  test.setTimeout(180_000);
  await login(
    page,
    accounts().find((a) => a.email === 'void@sieciowiec.xyz')!,
  );
  const session = await (await page.request.get('/api/auth/me')).json();
  const headers = { Origin: new URL(page.url()).origin, 'X-CSRF-Token': session.csrf_token };
  const catalog = (await (await page.request.get('/api/catalog/materials')).json()).materials;
  for (const id of ['GFA00', 'GFG02', 'GFB01', 'GFN03', 'GFU01'])
    expect(catalog.find((m: { id: string }) => m.id === id)).toBeTruthy();
  expect(catalog.every((m: { id: string }) => !m.id.startsWith('GFS'))).toBe(true);
  const file = await page.request.post('/api/models', {
    headers: { ...headers, 'X-Model-Filename': 'cube_20mm.stl' },
    data: readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl')),
  });
  expect(file.status()).toBe(201);
  const model = await file.json();
  for (const [material_id, color_code] of [
    ['GFA00', '10101'],
    ['GFG02', '33102'],
    ['GFB01', '45101'],
    ['GFN03', 'black'],
    ['GFU01', '50101'],
  ]) {
    const response = await page.request.post('/api/quotes/estimate', {
      headers,
      data: {
        model_id: model.id,
        material_id,
        color_code,
        options: { quantity: 1, quality: 'standard', wall_loops: 3, infill_percent: 20 },
      },
    });
    expect(response.status()).toBe(202);
    const job = await response.json();
    let result: any;
    await expect
      .poll(
        async () => {
          result = await (await page.request.get(job.poll_url)).json();
          return result.status;
        },
        { timeout: 90_000, intervals: [300, 1000] },
      )
      .toBe('succeeded');
    expect(result.result.id).toMatch(/^[0-9a-f-]{36}$/);
  }
  const body = {
    request_id: randomUUID(),
    parts: [
      {
        model_id: model.id,
        quantity: 2,
        material_id: 'GFG02',
        color_code: '',
        requested_color: 'PLA Silk · kolory z palety · #AA22FF',
        options: { quality: 'standard', wall_loops: 3, infill_percent: 20 },
      },
    ],
    contact: {
      name: 'Test Rapidmaker',
      email: 'void@sieciowiec.xyz',
      phone: '500600700',
      country: 'PL',
    },
    delivery: { method: 'pickup' },
    notes: 'Preferencja koloru wymaga potwierdzenia, bez podmiany materiału.',
  };
  const invalid = await page.request.post('/api/account/manual-quotes', {
    headers,
    data: { ...body, parts: [{ ...body.parts[0], requested_color: 'x'.repeat(201) }] },
  });
  expect(invalid.status()).toBe(400);
  const created = await page.request.post('/api/account/manual-quotes', { headers, data: body });
  expect(created.status()).toBe(200);
  const id = (await created.json()).order_id;
  const order = await (await page.request.get(`/api/account/orders/${id}`)).json();
  expect(order.amount_grosze).toBeNull();
  expect(order.manual_offer.state).toBe('awaiting_quote');
  const files = await (await page.request.get(`/api/account/orders/${id}/files`)).json();
  expect(files.files[0].material_id).toBe('GFG02');
  expect(files.files[0].color_code).toBe(body.parts[0].requested_color);
  const other = await browser.newContext();
  try {
    expect((await other.request.get(`/api/account/orders/${id}`)).status()).toBe(401);
  } finally {
    await other.close();
  }
});
