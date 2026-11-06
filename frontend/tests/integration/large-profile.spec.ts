import { expect, test } from './fixtures';
import { createHash } from 'node:crypto';
import { readFileSync, realpathSync, statSync } from 'node:fs';
import { basename, dirname, resolve, sep } from 'node:path';

type CorpusCase = { case: number; file: string; sha256: string; size_bytes: number };
const expectedCases = [
  { case: 8, triangles: 1_567_758, dimensions: [100, 190, 58] },
  { case: 20, triangles: 1_487_438, dimensions: [106.84888, 115.00562, 129] },
  { case: 29, triangles: 1_993_410, dimensions: [54.999817, 55.22171, 65.87768] },
  { case: 40, triangles: 1_677_324, dimensions: [80.94293, 77.361206, 100] },
];
const hash = (bytes: Buffer) => createHash('sha256').update(bytes).digest('hex');
const headers = { Origin: 'http://localhost:18437', 'Content-Type': 'application/octet-stream' };

function assertExactOrientedFaces(canonical: Buffer, glb: Buffer, count: number) {
  const document = JSON.parse(glb.subarray(20, 20 + glb.readUInt32LE(12)).toString('utf8'));
  const binary = 28 + glb.readUInt32LE(12);
  const faces = Buffer.alloc(count * 36);
  let offset = 0;
  for (const primitive of document.meshes[0].primitives) {
    const positions = document.accessors[primitive.attributes.POSITION];
    const indices = document.accessors[primitive.indices];
    const positionStart = binary + document.bufferViews[positions.bufferView].byteOffset;
    const indexStart = binary + document.bufferViews[indices.bufferView].byteOffset;
    expect(positions.componentType).toBe(5126);
    expect(positions.type).toBe('VEC3');
    expect([5123, 5125]).toContain(indices.componentType);
    const width = indices.componentType === 5123 ? 2 : 4;
    for (let i = 0; i < indices.count; i++) {
      const address = indexStart + i * width;
      const index = width === 2 ? glb.readUInt16LE(address) : glb.readUInt32LE(address);
      if (
        index >= positions.count ||
        offset + 12 > faces.length ||
        positionStart + index * 12 + 12 > glb.length
      )
        throw new Error('Invalid position span while comparing oriented faces');
      // Copy raw f32 bits; do not share the product decoder or round coordinates.
      glb.copy(faces, offset, positionStart + index * 12, positionStart + index * 12 + 12);
      offset += 12;
    }
  }
  expect(offset).toBe(faces.length);
  const sourceOrder = Uint32Array.from({ length: count }, (_, i) => i);
  const previewOrder = Uint32Array.from(sourceOrder);
  const sorted = (bytes: Buffer, stride: number, base: number, order: Uint32Array) =>
    order.sort((left, right) =>
      bytes.compare(
        bytes,
        base + right * stride,
        base + right * stride + 36,
        base + left * stride,
        base + left * stride + 36,
      ),
    );
  sorted(canonical, 50, 96, sourceOrder);
  sorted(faces, 36, 0, previewOrder);
  for (let i = 0; i < count; i++) {
    const a = sourceOrder[i] * 50 + 96,
      b = previewOrder[i] * 36;
    if (canonical.compare(faces, b, b + 36, a, a + 36) !== 0)
      throw new Error('Preview changed an oriented triangle or its multiplicity');
  }
}

function corpusSource(caseNumber: number) {
  const manifest = process.env.E2E_CORPUS_MANIFEST;
  if (!manifest) throw new Error('--large-models requires E2E_CORPUS_MANIFEST');
  const entries: CorpusCase[] = JSON.parse(readFileSync(manifest, 'utf8'));
  expect(Array.isArray(entries)).toBe(true);
  const matches = entries.filter((entry) => entry.case === caseNumber);
  expect(matches).toHaveLength(1);
  const item = matches[0];
  expect(item.sha256).toMatch(/^[a-f0-9]{64}$/);
  expect(Number.isSafeInteger(item.size_bytes) && item.size_bytes > 0).toBe(true);
  expect(item.size_bytes).toBeLessThanOrEqual(96 * 1024 * 1024);
  const root = realpathSync(dirname(resolve(manifest)));
  const path = realpathSync(resolve(root, item.file));
  expect(path.startsWith(root + sep), 'Corpus source stays inside manifest directory').toBe(true);
  expect(path.toLowerCase()).toMatch(/\.(stl|3mf)$/);
  expect(statSync(path).isFile()).toBe(true);
  expect(statSync(path).size).toBe(item.size_bytes);
  const bytes = readFileSync(path);
  expect(bytes.length).toBe(item.size_bytes);
  expect(hash(bytes)).toBe(item.sha256);
  return { item, path, bytes };
}

test.describe('opt-in large model API profile', () => {
  test.skip(process.env.RAPIDMAKER_LARGE_MODELS !== '1', 'Requires explicit --large-models');
  for (const expected of expectedCases) {
    test(`corpus ${expected.case}: large canonical STL and bounded private GLB preserve full geometry`, async ({
      request,
      playwright,
    }) => {
      test.setTimeout(240_000);
      const database = process.env.RAPIDMAKER_TEST_DATABASE ?? '';
      const modelRoot = process.env.RAPIDMAKER_TEST_MODEL_ROOT ?? '';
      expect(database).toMatch(/^rapidmaker_test_browser_[a-f0-9]+$/);
      expect(modelRoot).toMatch(/^\/tmp\/rapidmaker-browser-[^/]+\/models$/);
      const { item, path, bytes } = corpusSource(expected.case);
      const upload = await request.post('/api/models', {
        headers: { ...headers, 'x-model-filename': encodeURIComponent(basename(path)) },
        data: bytes,
        timeout: 90_000,
      });
      expect(upload.status()).toBe(201);
      const model = await upload.json();
      expect(model.id).toMatch(/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/);
      const metadata = await request.get(`/api/models/${model.id}`);
      expect(metadata.status()).toBe(200);
      expect(await metadata.json()).toEqual(model);
      for (let axis = 0; axis < 3; axis++)
        expect(Math.abs(model.dimensions_mm[axis] - expected.dimensions[axis])).toBeLessThanOrEqual(
          1e-3,
        );
      const sourceUrl = `/api/models/${model.id}/source`;
      const source = await request.get(sourceUrl);
      expect(source.status()).toBe(200);
      const original = await source.body();
      expect(original.length).toBe(item.size_bytes);
      expect(hash(original)).toBe(item.sha256);
      expect(source.headers()['cache-control'].split(/\s*,\s*/)).toContain('no-store');
      expect(model.preview_url).toBe(`/api/models/${model.id}/preview.glb`);
      const preview = await request.get(model.preview_url);
      expect(preview.status()).toBe(200);
      const glb = await preview.body();
      expect(glb.length).toBe(model.preview_bytes);
      expect(glb.length).toBeLessThan(25 * 1024 * 1024);
      expect(glb.readUInt32LE(0)).toBe(0x46546c67);
      expect(glb.readUInt32LE(4)).toBe(2);
      expect(glb.readUInt32LE(8)).toBe(glb.length);
      expect(glb.readUInt32LE(16)).toBe(0x4e4f534a);
      const jsonLength = glb.readUInt32LE(12);
      expect(jsonLength).toBeLessThanOrEqual(64 * 1024);
      const gltf = JSON.parse(glb.subarray(20, 20 + jsonLength).toString('utf8'));
      expect(glb.readUInt32LE(24 + jsonLength)).toBe(0x004e4942);
      expect(glb.readUInt32LE(20 + jsonLength) + jsonLength + 28).toBe(glb.length);
      expect(gltf.buffers).toEqual([{ byteLength: glb.length - jsonLength - 28 }]);
      expect(gltf.nodes).toEqual([
        { mesh: 0, scale: [0.001, 0.001, 0.001], rotation: [-0.70710678, 0, 0, 0.70710678] },
      ]);
      expect(gltf.scene).toBe(0);
      expect(gltf.scenes).toEqual([{ nodes: [0] }]);
      expect(gltf.meshes).toHaveLength(1);
      for (const field of ['images', 'textures', 'extensionsRequired'])
        expect(gltf[field] ?? []).toHaveLength(0);
      let indexCount = 0;
      for (const primitive of gltf.meshes[0].primitives) {
        const indices = gltf.accessors[primitive.indices];
        expect([5123, 5125]).toContain(indices.componentType);
        expect(indices.count % 3).toBe(0);
        indexCount += indices.count;
      }
      expect(indexCount).toBe(expected.triangles * 3);

      const root = realpathSync(modelRoot);
      const canonicalPath = realpathSync(resolve(root, model.id, 'canonical.stl'));
      expect(canonicalPath.startsWith(root + sep)).toBe(true);
      const canonical = readFileSync(canonicalPath);
      expect(canonical.length).toBeGreaterThan(50 * 1024 * 1024);
      expect(canonical.readUInt32LE(80)).toBe(expected.triangles);
      expect(canonical.length).toBe(84 + expected.triangles * 50);
      const min = [Infinity, Infinity, Infinity],
        max = [-Infinity, -Infinity, -Infinity];
      for (let triangle = 0; triangle < expected.triangles; triangle++)
        for (let corner = 0; corner < 3; corner++)
          for (let axis = 0; axis < 3; axis++) {
            const value = canonical.readFloatLE(84 + triangle * 50 + 12 + corner * 12 + axis * 4);
            if (!Number.isFinite(value))
              throw new Error('Canonical STL has a non-finite coordinate');
            min[axis] = Math.min(min[axis], value);
            max[axis] = Math.max(max[axis], value);
          }
      for (let axis = 0; axis < 3; axis++)
        expect(Math.abs(max[axis] - min[axis] - expected.dimensions[axis])).toBeLessThanOrEqual(
          1e-3,
        );

      if (expected.case === 29) {
        assertExactOrientedFaces(canonical, glb, expected.triangles);
        test
          .info()
          .annotations.push({
            type: 'exact-oriented-faces',
            description: `${expected.triangles} full raw-f32 oriented triangles match the canonical STL, including multiplicity`,
          });
      }

      if ([20, 29].includes(expected.case))
        await test.step('Large canonical mesh reaches the real slicer and price', async () => {
          const catalog = await (await request.get('/api/catalog/materials')).json();
          const material = catalog.materials.find((item: { id: string }) => item.id === 'GFA00');
          expect(material).toBeDefined();
          const submitted = await request.post('/api/quotes/estimate', {
            headers: { Origin: headers.Origin },
            data: {
              model_id: model.id,
              material_id: material.id,
              color_code: material.colors[0].code,
              options: { quantity: 1, quality: 'standard', wall_loops: 3, infill_percent: 20 },
            },
          });
          expect(submitted.status()).toBe(202);
          let job = await submitted.json();
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
          expect(job.status, job.message ?? 'Large model must get a measured price').toBe(
            'succeeded',
          );
          const { measurement, price } = job.result;
          expect(measurement.filament_grams).toBeGreaterThan(0);
          expect(measurement.time_seconds).toBeGreaterThan(0);
          expect(measurement.plates).toBeGreaterThan(0);
          // Independent invoice with this disposable runner's explicit fixture rates.
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
            type: 'large-measured-price',
            description: `case${expected.case}: ${measurement.plates} plates; ${price.gross_grosze} grosze; explicit fixture costs, not production profitability`,
          });
        });

      const outsider = await playwright.request.newContext({ baseURL: 'http://localhost:18437' });
      try {
        for (const url of [sourceUrl, model.preview_url])
          expect((await outsider.get(url)).status()).toBe(401);
        // Establish a different real guest owner, then verify ownership remains private.
        const otherUpload = await outsider.post('/api/models', {
          headers: { ...headers, 'x-model-filename': 'cube_20mm.stl' },
          data: readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl')),
        });
        expect(otherUpload.status()).toBe(201);
        for (const url of [sourceUrl, model.preview_url])
          expect((await outsider.get(url)).status()).toBe(404);
      } finally {
        await outsider.dispose();
      }
    });
  }
});
