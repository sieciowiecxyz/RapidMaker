import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { randomUUID } from 'node:crypto';

test('real slicer and Furgonetka sandbox return priced delivery and reuse a valid offer', async ({
  request,
}) => {
  test.skip(process.env.RAPIDMAKER_SANDBOX !== '1', 'Requires --sandbox; never books a shipment');
  test.setTimeout(120_000);
  const origin = { Origin: 'http://localhost:18437' };
  const catalog = await (await request.get('/api/catalog/materials')).json();
  const pla = catalog.materials.find((m: { id: string }) => m.id === 'GFA00');
  const upload = await request.post('/api/models', {
    headers: { ...origin, 'x-model-filename': 'cube_20mm.stl' },
    data: readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl')),
  });
  expect(upload.status()).toBe(201);
  const model = await upload.json();
  const estimate = await request.post('/api/quotes/estimate', {
    headers: origin,
    data: {
      model_id: model.id,
      material_id: 'GFA00',
      color_code: pla.colors[0].code,
      options: { quantity: 10, quality: 'standard', wall_loops: 3, infill_percent: 20 },
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
  const body = {
    parts: [{ id: randomUUID(), quantity: 10, analysis_id: model.id, quote_id: result.result.id }],
    delivery: 'courier',
    contact: {
      name: 'Test Rapidmaker',
      email: 'void@sieciowiec.xyz',
      phone: '500600700',
      street: 'Testowa 1',
      postalCode: '05-500',
      city: 'Piaseczno',
      pickupPoint: '',
      company: '',
      nip: '',
      emailOrderUpdates: false,
    },
    manual: false,
  };
  const response = await request.post('/api/checkout/quote', { headers: origin, data: body });
  expect(response.status()).toBe(200);
  const quote = await response.json();
  expect(quote.offers.length).toBeGreaterThan(0);
  expect(quote.packing.parcels.length).toBeGreaterThan(0);
  expect(
    quote.offers.every(
      (offer: { price_grosze: number }) =>
        Number.isSafeInteger(offer.price_grosze) && offer.price_grosze > 0,
    ),
  ).toBe(true);
  expect(quote.offers[0].price_grosze).toBe(
    Math.min(...quote.offers.map((o: { price_grosze: number }) => o.price_grosze)),
  );
  // Repeating quote for this same draft/version must reuse the saved valid snapshot.
  const draft = await (await request.get(`/api/checkout/drafts/${quote.id}`)).json();
  const cached = await request.post(`/api/checkout/drafts/${quote.id}/quote`, {
    headers: origin,
    data: { version: draft.version },
  });
  expect(cached.status()).toBe(200);
  const reused = await cached.json();
  expect(reused.quote.offers).toEqual(draft.quote.offers);
  expect(reused.version).toBe(draft.version);
});
