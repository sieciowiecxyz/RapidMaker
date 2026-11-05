import { expect, test } from './fixtures';
import { createHash } from 'node:crypto';
import { readFileSync, realpathSync, statSync } from 'node:fs';
import { dirname, resolve, sep } from 'node:path';

type ModelCase = { case: number; file: string; sha256: string; size_bytes: number };
// Opt-in only: normal integration runs never silently process third-party downloads.
const manifest = process.env.E2E_CORPUS_MANIFEST;
const cases: ModelCase[] = manifest ? JSON.parse(readFileSync(manifest, 'utf8')) : [];
const limit = Number(process.env.E2E_CORPUS_LIMIT ?? 100);
if (!Number.isInteger(limit) || limit < 1 || limit > 100) throw new Error('Invalid corpus limit');

for (const item of cases.slice(0, limit)) {
  test(`corpus ${item.case}: real model pipeline`, async ({ request }) => {
    test.setTimeout(240_000);
    const root = realpathSync(dirname(resolve(manifest!)));
    const path = realpathSync(resolve(root, item.file));
    expect(path.startsWith(root + sep)).toBe(true);
    expect(statSync(path).size).toBeLessThanOrEqual(128 * 1024 * 1024);
    const bytes = readFileSync(path);
    expect(bytes.length).toBe(item.size_bytes);
    expect(createHash('sha256').update(bytes).digest('hex')).toBe(item.sha256);
    const upload = await request.post('/api/models', {
      headers: {
        Origin: 'http://localhost:18437',
        'Content-Type': 'application/octet-stream',
        'x-model-filename': encodeURIComponent(path.split(sep).at(-1)!),
      },
      data: bytes,
      timeout: 90_000,
    });
    if (bytes.length > 32 * 1024 * 1024) {
      expect(upload.status()).toBe(413);
      test
        .info()
        .annotations.push({ type: 'upload-limit', description: 'Expected HTTP 413 above 32 MiB' });
      return;
    }
    if (upload.status() === 422 && (await upload.json()).code === 'model_resource_limit')
      test.info().annotations.push({
        type: 'resource-limit',
        description: 'Upload rejected by geometry resource budget; not an automatic quote',
      });
    expect(upload.status(), `Upload response: ${await upload.text()}`).toBe(201);
    const model = await upload.json();
    expect(model.dimensions_mm).toHaveLength(3);
    expect(model.dimensions_mm.every((n: number) => Number.isFinite(n) && n > 0)).toBe(true);
    const preview = await request.get(model.preview_url);
    expect(preview.status()).toBe(200);
    const glb = await preview.body();
    expect(glb.subarray(0, 4).toString()).toBe('glTF');
    expect(glb.readUInt32LE(8)).toBe(glb.length);
    const catalog = await (await request.get('/api/catalog/materials')).json();
    const material = catalog.materials.find((m: { id: string }) => m.id === 'GFA00');
    const quote = await request.post('/api/quotes/estimate', {
      headers: { Origin: 'http://localhost:18437' },
      data: {
        model_id: model.id,
        material_id: material.id,
        color_code: material.colors[0].code,
        options: { quantity: 1, quality: 'standard', wall_loops: 3, infill_percent: 20 },
      },
    });
    if (quote.status() === 422) {
      const error = await quote.json();
      expect(error.code).toBe('manual_quote_required');
      expect(['size_limit', 'printer_fit']).toContain(error.reason);
      test.info().annotations.push({ type: 'manual-quote', description: error.reason });
      return;
    }
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
    expect(job.status, job.message ?? 'Slicer must produce a measured quote').toBe('succeeded');
    const { measurement, price } = job.result;
    expect(measurement.filament_grams).toBeGreaterThan(0);
    expect(measurement.time_seconds).toBeGreaterThan(0);
    for (const amount of Object.values(price)) {
      expect(Number.isSafeInteger(amount)).toBe(true);
      expect(amount).toBeGreaterThanOrEqual(0);
    }
    // Disposable runner explicitly sets PLA purchase cost to 10000 net grosze/kg.
    expect(price.filament_grosze).toBe(
      Math.ceil(Math.ceil(measurement.filament_grams * 1000) / 100),
    );
    expect(price.cost_net_grosze).toBeGreaterThanOrEqual(
      price.filament_grosze + price.energy_grosze,
    );
    expect(price.net_grosze).toBeGreaterThanOrEqual(price.cost_net_grosze);
    expect(price.gross_grosze).toBe(price.net_grosze + price.vat_grosze);
    test.info().annotations.push({
      type: 'measured-price',
      description: 'Real slicer and price assertions completed',
    });
  });
}
