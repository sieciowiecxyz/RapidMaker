import { expect, test } from './fixtures';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { execFileSync } from 'node:child_process';

test('concurrent guests deduplicate their own slicer jobs without sharing models or prices', async ({
  playwright,
}) => {
  test.setTimeout(240_000);
  const clients = await Promise.all(
    Array.from({ length: 4 }, () =>
      playwright.request.newContext({ baseURL: 'http://localhost:18437' }),
    ),
  );
  const headers = { Origin: 'http://localhost:18437' };
  try {
    const models: { id: string; preview_url: string }[] = [];
    // Upload sequentially: this test exercises the quote queue, not conversion admission limits.
    for (const client of clients) {
      const uploaded = await client.post('/api/models', {
        headers: {
          ...headers,
          'Content-Type': 'application/octet-stream',
          'x-model-filename': 'cube_20mm.stl',
        },
        data: readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl')),
      });
      expect(uploaded.status()).toBe(201);
      models.push(await uploaded.json());
    }
    expect(new Set(models.map((model) => model.id)).size).toBe(clients.length);
    for (const model of models)
      expect(model.id).toMatch(/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/);
    const modelIds = models.map((model) => `'${model.id}'`).join(',');
    const ownJobs = `FROM slicer_jobs WHERE model_id IN (${modelIds})`;
    const catalog = await clients[0].get('/api/catalog/materials');
    expect(catalog.status()).toBe(200);
    const material = (await catalog.json()).materials.find(
      (value: { id: string }) => value.id === 'GFA00',
    );
    expect(material?.colors.length).toBeGreaterThan(0);
    const input = (index: number, modelId = models[index].id) => ({
      model_id: modelId,
      material_id: material.id,
      color_code: material.colors[0].code,
      options: { quantity: index + 1, quality: 'standard', wall_loops: 3, infill_percent: 20 },
    });
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
    const started = Date.now();
    const pending = clients.map((client, index) =>
      Promise.all(
        Array.from({ length: 2 }, () =>
          client.post('/api/quotes/estimate', { headers, data: input(index) }),
        ),
      ),
    );
    const heartbeat = Date.now();
    expect((await clients[0].get('/api/catalog/materials')).status()).toBe(200);
    expect(Date.now() - heartbeat).toBeLessThan(2000);
    const pairs = await Promise.all(pending);
    expect(Date.now() - started).toBeLessThan(5000);
    const jobs = [];
    for (const pair of pairs) {
      expect(pair.map((response) => response.status())).toEqual([202, 202]);
      const [first, duplicate] = await Promise.all(pair.map((response) => response.json()));
      expect(duplicate.id).toBe(first.id);
      jobs.push(first);
    }
    expect(new Set(jobs.map((job) => job.id)).size).toBe(clients.length);
    expect(jobs.some((job) => ['queued', 'running'].includes(job.status))).toBe(true);

    await expect
      .poll(() => Number(sql(`SELECT count(*) ${ownJobs} AND status='running'`)), {
        timeout: 10_000,
        intervals: [100],
      })
      .toBe(1);
    const duringSlice = Date.now();
    expect((await clients[0].get('/api/catalog/materials')).status()).toBe(200);
    expect(Date.now() - duringSlice).toBeLessThan(2000);
    const results = await Promise.all(
      clients.map(async (client, index) => {
        const foreign = (index + 1) % clients.length;
        for (const path of [
          `/api/models/${models[foreign].id}`,
          models[foreign].preview_url,
          jobs[foreign].poll_url,
        ])
          expect((await client.get(path)).status()).toBe(404);
        expect(
          (
            await client.post('/api/quotes/estimate', {
              headers,
              data: input(index, models[foreign].id),
            })
          ).status(),
        ).toBe(404);
        let job = jobs[index];
        await expect
          .poll(
            async () => {
              const response = await client.get(job.poll_url);
              expect(response.status()).toBe(200);
              job = await response.json();
              return job.status;
            },
            { timeout: 180_000, intervals: [250, 1000] },
          )
          .toMatch(/^(succeeded|failed)$/);
        expect(job.status, job.message ?? 'Real slicer must complete the owned job').toBe(
          'succeeded',
        );
        expect(job.result).toMatchObject({
          id: jobs[index].id,
          model_id: models[index].id,
          quantity: index + 1,
          currency: 'PLN',
          options: input(index).options,
        });
        expect(job.result.measurement.quantity).toBe(index + 1);
        expect(job.result.measurement.time_seconds).toBeGreaterThan(0);
        expect(job.result.measurement.filament_grams).toBeGreaterThan(0);
        for (const amount of Object.values(job.result.price)) {
          expect(Number.isSafeInteger(amount)).toBe(true);
          expect(amount).toBeGreaterThanOrEqual(0);
        }
        expect(job.result.price.gross_grosze).toBeGreaterThan(0);
        return job.result;
      }),
    );
    // Every foreign snapshot now exists: rejection must prove ownership, not a pending result.
    for (const [index, client] of clients.entries()) {
      const foreign = (index + 1) % clients.length;
      const checkout = (quoteId: string, modelIndex = index) =>
        client.post('/api/checkout/quote', {
          headers,
          data: {
            delivery: 'pickup',
            parts: [
              {
                id: models[modelIndex].id,
                analysis_id: models[modelIndex].id,
                quantity: modelIndex + 1,
                quote_id: quoteId,
              },
            ],
          },
        });
      expect((await checkout(jobs[foreign].id, foreign)).status()).toBe(409);
      const owned = await checkout(results[index].id);
      expect(owned.status()).toBe(200);
      expect((await owned.json()).print_grosze).toBe(results[index].price.gross_grosze);
      const cached = await client.post('/api/quotes/estimate', { headers, data: input(index) });
      expect(cached.status()).toBe(202);
      expect(await cached.json()).toMatchObject({
        id: jobs[index].id,
        status: 'succeeded',
        result: results[index],
      });
    }
    expect(
      sql(
        `SELECT (SELECT count(*) ${ownJobs}), (SELECT count(*) FROM quotes WHERE request->>'model_id' IN (${modelIds})), max(attempts) ${ownJobs}`,
      ),
    ).toBe('4|4|1');
  } finally {
    await Promise.all(clients.map((client) => client.dispose()));
  }
});
