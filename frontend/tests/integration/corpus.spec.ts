import { expect, test } from './fixtures';
import { createHash } from 'node:crypto';
import { readFileSync, realpathSync, statSync } from 'node:fs';
import { dirname, resolve, sep } from 'node:path';

type ModelCase = { case: number; file: string; sha256: string; size_bytes: number };
// Opt-in only: default integration never processes third-party downloads.
const manifest = process.env.E2E_CORPUS_MANIFEST;
const cases: ModelCase[] = manifest ? JSON.parse(readFileSync(manifest, 'utf8')) : [];
if (
  manifest &&
  (!Array.isArray(cases) ||
    cases.length !== 41 ||
    new Set(cases.map((item) => item.case)).size !== 41 ||
    cases.some(
      (item) =>
        !Number.isInteger(item.case) ||
        item.case < 1 ||
        item.case > 41 ||
        typeof item.file !== 'string' ||
        !/\.(stl|3mf)$/i.test(item.file) ||
        !/^[a-f0-9]{64}$/.test(item.sha256) ||
        !Number.isSafeInteger(item.size_bytes) ||
        item.size_bytes <= 0,
    ))
)
  throw new Error('Corpus requires the 41 accepted unique source cases with valid provenance.');
const large = process.env.RAPIDMAKER_LARGE_MODELS === '1';
const capMiB = large ? 96 : 32;
const limit = Number(process.env.E2E_CORPUS_LIMIT ?? 100);
if (!Number.isInteger(limit) || limit < 1 || limit > 100) throw new Error('Invalid corpus limit');

for (const item of cases.slice(0, limit)) {
  test(`corpus ${item.case}: real model pipeline`, async ({ request }) => {
    test.setTimeout(240_000);
    const root = realpathSync(dirname(resolve(manifest!)));
    const path = realpathSync(resolve(root, item.file));
    expect(path.startsWith(root + sep)).toBe(true);
    expect(statSync(path).isFile()).toBe(true);
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
    if (bytes.length > capMiB * 1024 * 1024) {
      expect(upload.status()).toBe(413);
      test.info().annotations.push({
        type: 'upload-limit',
        description: `Expected HTTP 413 above ${capMiB} MiB; not processed`,
      });
      return;
    }
    // Independently diagnosed source defects/budgets; unexpected 422/503 still fail.
    const topology = [14, 21, 37].includes(item.case);
    const resource = item.case === 41 || (!large && [8, 19, 20, 29, 40].includes(item.case));
    if (topology || resource || item.case === 32) {
      expect(upload.status()).toBe(422);
      const error = await upload.json();
      if (item.case === 32)
        expect(error).toEqual({
          message: 'Nie można odczytać geometrii. Wybierz poprawny STL, STEP lub 3MF.',
        });
      else if (!large && item.case === 14) {
        // Standard budget can take precedence over its independently known bad topology.
        expect(['model_resource_limit', 'model_topology_invalid']).toContain(error.code);
      } else expect(error.code).toBe(topology ? 'model_topology_invalid' : 'model_resource_limit');
      test.info().annotations.push({
        type: error.code === 'model_resource_limit' ? 'resource-limit' : 'invalid-source',
        description: `${error.code ?? 'malformed-geometry'}; source rejected, no preview or price`,
      });
      return;
    }
    expect(upload.status(), `Upload response: ${await upload.text()}`).toBe(201);
    const model = await upload.json();
    expect(model.dimensions_mm).toHaveLength(3);
    expect(model.dimensions_mm.every((n: number) => Number.isFinite(n) && n > 0)).toBe(true);
    expect(model.preview_url).toBe(`/api/models/${model.id}/preview.glb`);
    const preview = await request.get(model.preview_url);
    expect(preview.status()).toBe(200);
    const glb = await preview.body();
    expect(glb.length).toBeGreaterThanOrEqual(32);
    expect(glb.length).toBeLessThanOrEqual(25 * 1024 * 1024);
    expect(glb.subarray(0, 4).toString()).toBe('glTF');
    expect(glb.readUInt32LE(4)).toBe(2);
    expect(glb.readUInt32LE(8)).toBe(glb.length);
    const jsonLength = glb.readUInt32LE(12);
    expect(jsonLength).toBeLessThanOrEqual(64 * 1024);
    expect(glb.readUInt32LE(16)).toBe(0x4e4f534a);
    expect(glb.readUInt32LE(24 + jsonLength)).toBe(0x004e4942);
    expect(glb.readUInt32LE(20 + jsonLength)).toBe(glb.length - 28 - jsonLength);
    const document = JSON.parse(glb.subarray(20, 20 + jsonLength).toString('utf8'));
    expect(document.nodes[0]).toMatchObject({
      scale: [0.001, 0.001, 0.001],
      rotation: [-0.70710678, 0, 0, 0.70710678],
    });
    const primitives = document.meshes[0].primitives;
    expect(primitives.length).toBeGreaterThan(0);
    expect(primitives.length).toBeLessThanOrEqual(64);
    const triangles = primitives.reduce((sum: number, primitive: { indices: number }) => {
      const count = document.accessors[primitive.indices].count;
      expect(Number.isSafeInteger(count) && count > 0 && count % 3 === 0).toBe(true);
      return sum + count / 3;
    }, 0);
    expect(triangles).toBeLessThanOrEqual(2_000_000);
    const approximation = document.nodes[0].extras?.rapidmakerPreview;
    if (approximation) {
      expect(Number.isSafeInteger(approximation.source_triangles)).toBe(true);
      expect(approximation.source_triangles).toBeGreaterThan(triangles);
      expect(approximation.source_triangles).toBeLessThanOrEqual(4_194_302);
      expect(approximation.display_triangles).toBe(triangles);
      expect(approximation.max_error_mm).toBe(0.05);
      expect(Number.isFinite(approximation.reported_error_mm)).toBe(true);
      expect(approximation.reported_error_mm).toBeGreaterThanOrEqual(0);
      expect(approximation.reported_error_mm).toBeLessThanOrEqual(0.05);
    }
    test.info().annotations.push({
      type: 'preview-ready',
      description: `${triangles} display triangles; ${approximation ? 'approximate display only' : 'exact display'}; not a measured price`,
    });
    const catalog = await (await request.get('/api/catalog/materials')).json();
    const material = catalog.materials.find((m: { id: string }) => m.id === 'GFA00');
    expect(material).toBeDefined();
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
      // Independently read raw POSITION bounds; do not trust a manual reason alone.
      const min = [Infinity, Infinity, Infinity],
        max = [-Infinity, -Infinity, -Infinity];
      const binary = jsonLength + 28;
      for (const primitive of primitives) {
        const positions = document.accessors[primitive.attributes.POSITION];
        const view = document.bufferViews[positions.bufferView];
        expect(positions.componentType).toBe(5126);
        expect(positions.type).toBe('VEC3');
        expect(Number.isSafeInteger(positions.count) && positions.count > 0).toBe(true);
        expect(view.byteLength).toBe(positions.count * 12);
        const start = binary + (view.byteOffset ?? 0);
        expect(start + view.byteLength).toBeLessThanOrEqual(glb.length);
        for (let vertex = 0; vertex < positions.count; vertex++)
          for (let axis = 0; axis < 3; axis++) {
            const value = glb.readFloatLE(start + vertex * 12 + axis * 4);
            if (!Number.isFinite(value)) throw new Error('Preview has non-finite coordinates');
            min[axis] = Math.min(min[axis], value);
            max[axis] = Math.max(max[axis], value);
          }
      }
      const dimensions = min.map((value, axis) => Math.fround(max[axis] - value));
      for (let axis = 0; axis < 3; axis++)
        expect(Math.abs(dimensions[axis] - model.dimensions_mm[axis])).toBeLessThanOrEqual(
          Math.max(1e-3, dimensions[axis] * 2e-7),
        );
      const [short, middle, long] = [...dimensions].sort((a, b) => a - b);
      // An orthogonal P1P layout needs the short footprint edge <=238, height <=250, long edge <=256.
      const fitsPrinter = short <= 238 && middle <= 250 && long <= 256;
      expect(error.reason).toBe(long > 500 ? 'size_limit' : fitsPrinter ? null : 'printer_fit');
      test
        .info()
        .annotations.push({
          type: 'manual-quote',
          description: `${error.reason}; GLB bounds ${dimensions.join('x')} mm independently justify manual route`,
        });
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
    expect(Number.isFinite(measurement.filament_grams)).toBe(true);
    expect(Number.isSafeInteger(measurement.time_seconds)).toBe(true);
    expect(measurement.time_seconds).toBeGreaterThan(0);
    expect(Number.isSafeInteger(measurement.plates) && measurement.plates > 0).toBe(true);
    for (const amount of Object.values(price)) {
      expect(Number.isSafeInteger(amount)).toBe(true);
      expect(amount).toBeGreaterThanOrEqual(0);
    }
    // Independent invoice using the disposable runner's explicit fixture rates.
    const components = {
      filament_grosze: Math.ceil(Math.ceil(measurement.filament_grams * 1000) / 100),
      machine_grosze: Math.ceil(measurement.time_seconds / 18),
      energy_grosze: Math.ceil(measurement.time_seconds / 200),
      handling_grosze: measurement.plates * 500,
      setup_grosze: 1000,
    };
    expect(price).toMatchObject(components);
    const base = Object.values(components).reduce((a, b) => a + b, 0);
    const loss = Math.ceil(base / 20),
      cost = base + loss;
    const net = Math.ceil(Math.max(2000, (cost * 10) / 7));
    const vat = Math.ceil((net * 23) / 100);
    expect(price).toMatchObject({
      loss_grosze: loss,
      cost_net_grosze: cost,
      net_grosze: net,
      vat_grosze: vat,
      gross_grosze: net + vat,
    });
    test.info().annotations.push({
      type: 'measured-price',
      description: `${measurement.filament_grams} g; ${measurement.time_seconds} s; ${measurement.plates} plates; ${price.gross_grosze} grosze; real slicer and complete fixture invoice, not production profitability`,
    });
  });
}
