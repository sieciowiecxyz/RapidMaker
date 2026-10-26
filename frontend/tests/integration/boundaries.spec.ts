import { expect, test } from './fixtures';

test('generated API documentation loads its real schema and expands an operation', async ({
  page,
}) => {
  const schema = await page.request.get('/api/openapi.json');
  expect(schema.status()).toBe(200);
  const api = await schema.json();
  expect(api.paths['/api/auth/start']).toBeTruthy();
  expect(api.paths['/api/legal/documents']).toBeTruthy();
  await page.goto('/api/');
  await expect(page.locator('.swagger-ui').first()).toBeVisible();
  await page.locator('.opblock-summary').first().click();
  await expect(page.locator('.opblock.is-open').first()).toBeVisible();
});

test('real packing preserves all quantities and rejects untrusted checkout fields/origins', async ({
  request,
}) => {
  const headers = { Origin: 'http://localhost:18437' };
  const parts = [
    { id: 'a', quantity: 1000, dimensions_mm: [40, 30, 20], weight_g: 10 },
    { id: 'b', quantity: 7, dimensions_mm: [100, 80, 60], weight_g: 80 },
  ];
  const packed = await request.post('/api/shipping/estimate', {
    headers,
    data: { parts, delivery: 'courier' },
  });
  expect(packed.status()).toBe(200);
  const result = await packed.json();
  expect(result.status).toBe('estimated');
  expect(result.parcels.length).toBeGreaterThan(0);
  expect(result.parcels.length).toBeLessThanOrEqual(30);
  const quantities = new Map<string, number>();
  for (const parcel of result.parcels) {
    expect(parcel.weight_g).toBeGreaterThan(0);
    expect(parcel.outer_mm.every((value: number) => value > 0)).toBe(true);
    for (const placement of parcel.placements)
      quantities.set(placement.part_id, (quantities.get(placement.part_id) ?? 0) + 1);
  }
  expect(Object.fromEntries(quantities)).toEqual({ a: 1000, b: 7 });
  const forged = await request.post('/api/checkout/intent', {
    headers,
    data: {
      quote_id: '00000000-0000-0000-0000-000000000001',
      offer_id: 'pickup',
      legal_version: '2026-10-25',
      amount: 1,
    },
  });
  expect(forged.status()).toBe(422);
  const crossOrigin = await request.post('/api/models', {
    headers: { Origin: 'https://example.invalid' },
    multipart: {
      file: {
        name: 'invalid.stl',
        mimeType: 'application/octet-stream',
        buffer: Buffer.from('invalid'),
      },
    },
  });
  expect(crossOrigin.status()).toBe(403);
  expect((await request.get('/api/account/orders')).status()).toBe(401);
  expect(
    (
      await request.get('/api/admin/orders', {
        headers: { Authorization: 'Bearer old-demo-token' },
      })
    ).status(),
  ).toBe(401);
});

test('a forged payment return cannot display a paid receipt or allow another payment', async ({
  page,
}) => {
  await page.goto('/zamowienie?payment_intent=pi_forged');
  await expect(page.getByRole('alert')).toBeVisible();
  await expect(page.getByRole('button', { name: /^Zamawiam i płacę/ })).toHaveCount(0);
  await expect(
    page.getByText(/Płatność otrzymana|Dziękujemy za opłacenie/, { exact: false }),
  ).toHaveCount(0);
});
