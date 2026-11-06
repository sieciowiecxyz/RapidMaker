import { expect, test } from '@playwright/test';
import { parsePreviewGlb } from '../../src/storefront/lib/glbParser';
import { MAX_PREVIEW_TRIANGLES, meshFromPositions } from '../../src/storefront/lib/stlParser';

const invalid = 'Nieprawidłowy format prywatnego podglądu GLB.';
const first = [0, 0, 0, 4, 0, 0, 0, 3, 2];
const second = [4, 0, 0, 4, 3, 2, 0, 3, 2];

// Independent canonical wire writer: no production encoder/decoder is used to construct fixtures.
function wire(chunks: { positions: number[]; indices: number[] }[], componentType = 5123) {
  const accessors: Record<string, unknown>[] = [];
  const bufferViews: Record<string, unknown>[] = [];
  const primitives: Record<string, unknown>[] = [];
  const parts: Buffer[] = [];
  let offset = 0;
  for (const chunk of chunks) {
    const p = accessors.length,
      i = p + 1;
    const positions = Buffer.alloc(chunk.positions.length * 4);
    chunk.positions.forEach((value, index) => positions.writeFloatLE(value, index * 4));
    const indexBytes = componentType === 5123 ? 2 : 4;
    const indices = Buffer.alloc(chunk.indices.length * indexBytes);
    chunk.indices.forEach((value, index) => {
      if (indexBytes === 2) indices.writeUInt16LE(value, index * 2);
      else indices.writeUInt32LE(value, index * 4);
    });
    bufferViews.push({ buffer: 0, byteOffset: offset, byteLength: positions.length });
    offset += positions.length;
    bufferViews.push({ buffer: 0, byteOffset: offset, byteLength: indices.length });
    offset += indices.length;
    const padding = Buffer.alloc((4 - (offset % 4)) % 4);
    offset += padding.length;
    parts.push(positions, indices, padding);
    accessors.push(
      { bufferView: p, componentType: 5126, count: chunk.positions.length / 3, type: 'VEC3' },
      { bufferView: i, componentType, count: chunk.indices.length, type: 'SCALAR' },
    );
    primitives.push({ attributes: { POSITION: p }, indices: i, mode: 4 });
  }
  return {
    json: {
      asset: { version: '2.0', generator: 'rapidmaker-geometry' },
      buffers: [{ byteLength: offset }],
      bufferViews,
      accessors,
      meshes: [{ primitives }],
      nodes: [{ mesh: 0, scale: [0.001, 0.001, 0.001], rotation: [-0.70710678, 0, 0, 0.70710678] }],
      scenes: [{ nodes: [0] }],
      scene: 0,
    },
    binary: Buffer.concat(parts),
  };
}
type Wire = ReturnType<typeof wire>;
function encode(json: unknown, binary: Buffer): ArrayBuffer {
  const text = Buffer.from(JSON.stringify(json));
  const jsonBytes = Buffer.concat([text, Buffer.alloc((4 - (text.length % 4)) % 4, 0x20)]);
  const bytes = Buffer.alloc(28 + jsonBytes.length + binary.length);
  bytes.writeUInt32LE(0x46546c67, 0);
  bytes.writeUInt32LE(2, 4);
  bytes.writeUInt32LE(bytes.length, 8);
  bytes.writeUInt32LE(jsonBytes.length, 12);
  bytes.writeUInt32LE(0x4e4f534a, 16);
  jsonBytes.copy(bytes, 20);
  bytes.writeUInt32LE(binary.length, 20 + jsonBytes.length);
  bytes.writeUInt32LE(0x004e4942, 24 + jsonBytes.length);
  binary.copy(bytes, 28 + jsonBytes.length);
  return Uint8Array.from(bytes).buffer;
}
const single = () => wire([{ positions: first, indices: [0, 1, 2] }]);
function rejects(input: ArrayBuffer) {
  let error: unknown;
  try {
    parsePreviewGlb(input);
  } catch (cause) {
    error = cause;
  }
  expect(error).toBeInstanceOf(Error);
  expect((error as Error).name).toBe('Error');
  expect((error as Error).message).toBe(invalid);
}

for (const component of [5125, 5123]) {
  test(`canonical ${component === 5125 ? 'legacy u32' : 'compact u16'} preserves coordinates, dimensions and winding`, () => {
    const fixture = wire([{ positions: first, indices: [0, 1, 2] }], component);
    const mesh = parsePreviewGlb(encode(fixture.json, fixture.binary));
    expect(mesh).toEqual(meshFromPositions(new Float32Array(first)));
    expect(mesh.dimensions).toEqual([4, 3, 2]);
    expect(mesh.triangleCount).toBe(1);
    // Z-up to Y-up: normal of A/B/C points toward positive Y and positive Z.
    expect(mesh.vertices[4]).toBeGreaterThan(0);
    expect(mesh.vertices[5]).toBeGreaterThan(0);
  });
}

test('compact chunks preserve order and winding through odd-index padding and zero-area faces', () => {
  const fixture = wire([
    { positions: first, indices: [0, 1, 2] },
    { positions: second, indices: [0, 1, 2, 0, 0, 0] },
  ]);
  expect(fixture.json.bufferViews[1].byteLength).toBe(6);
  expect(fixture.binary.subarray(42, 44)).toEqual(Buffer.alloc(2));
  const mesh = parsePreviewGlb(encode(fixture.json, fixture.binary));
  expect(mesh).toEqual(
    meshFromPositions(
      new Float32Array([
        ...first,
        ...second,
        ...second.slice(0, 3),
        ...second.slice(0, 3),
        ...second.slice(0, 3),
      ]),
    ),
  );
  expect(mesh.triangleCount).toBe(2);
  expect(mesh.dimensions).toEqual([4, 3, 2]);
});

const mutations: [string, (fixture: Wire) => void][] = [
  ['out-of-range index', (fixture) => fixture.binary.writeUInt16LE(3, 36)],
  ['reserved u16 index', (fixture) => fixture.binary.writeUInt16LE(65535, 36)],
  [
    'overlapping views',
    (fixture) => {
      fixture.json.bufferViews[1].byteOffset = 0;
    },
  ],
  [
    'out-of-range view',
    (fixture) => {
      fixture.json.bufferViews[1].byteOffset = 1000;
    },
  ],
  [
    'nonzero alignment padding',
    (fixture) => {
      fixture.binary[42] = 1;
    },
  ],
  [
    'external buffer URI',
    (fixture) => {
      Object.assign(fixture.json.buffers[0], { uri: 'https://example.invalid/model.bin' });
    },
  ],
  [
    'arbitrary transform',
    (fixture) => {
      Object.assign(fixture.json.nodes[0], { translation: [1, 0, 0] });
    },
  ],
  [
    'wrong fixed scale',
    (fixture) => {
      fixture.json.nodes[0].scale = [1, 1, 1];
    },
  ],
  [
    'extra node',
    (fixture) => {
      fixture.json.nodes.push(fixture.json.nodes[0]);
    },
  ],
  [
    'sparse positions',
    (fixture) => {
      fixture.json.accessors[0].sparse = {};
    },
  ],
  [
    'strided view',
    (fixture) => {
      fixture.json.bufferViews[0].byteStride = 12;
    },
  ],
  [
    'required extension',
    (fixture) => {
      Object.assign(fixture.json, { extensionsRequired: ['KHR_draco_mesh_compression'] });
    },
  ],
  [
    'non-triangle index count',
    (fixture) => {
      fixture.json.accessors[1].count = 2;
    },
  ],
  [
    'compact position overflow',
    (fixture) => {
      fixture.json.accessors[0].count = 65536;
    },
  ],
];
for (const [name, mutate] of mutations) {
  test(`canonical GLB rejects ${name}`, () => {
    const fixture = single();
    mutate(fixture);
    rejects(encode(fixture.json, fixture.binary));
  });
}

test('canonical GLB rejects truncated bytes and malformed JSON with a fixed error', () => {
  const fixture = single();
  const valid = encode(fixture.json, fixture.binary);
  rejects(valid.slice(0, -1));
  const badJson = valid.slice(0);
  new Uint8Array(badJson)[20] = 0x21;
  rejects(badJson);
  for (const json of [
    null,
    [],
    1,
    'text',
    { ...fixture.json, nodes: [null] },
    { ...fixture.json, accessors: [null, null] },
    { ...fixture.json, meshes: [{ primitives: {} }] },
    { ...fixture.json, buffers: { length: 1 } },
  ])
    rejects(encode(json, fixture.binary));
});

test('canonical GLB rejects more than 64 primitives and multiple legacy primitives', () => {
  for (const [count, component] of [
    [65, 5123],
    [2, 5125],
  ]) {
    const fixture = wire(
      Array.from({ length: count }, () => ({ positions: first, indices: [0, 1, 2] })),
      component,
    );
    rejects(encode(fixture.json, fixture.binary));
  }
});

test('aggregate compact triangle budget is checked across chunks before flattened allocation', () => {
  const count = 3 * (Math.floor(MAX_PREVIEW_TRIANGLES / 2) + 1);
  const fixture = wire(
    Array.from({ length: 2 }, () => ({ positions: first, indices: Array<number>(count).fill(0) })),
  );
  expect(fixture.binary.length).toBeLessThan(25 * 1024 * 1024);
  rejects(encode(fixture.json, fixture.binary));
});

test('compact GLB accepts 64 primitives and repeated chunk positions beyond the legacy source cap', () => {
  const boundary = wire(
    Array.from({ length: 64 }, () => ({ positions: first, indices: [0, 1, 2] })),
  );
  expect(parsePreviewGlb(encode(boundary.json, boundary.binary)).triangleCount).toBe(64);
  const positions = Array<number>(65535 * 3).fill(0);
  positions.splice(0, first.length, ...first);
  const repeated = wire(Array.from({ length: 16 }, () => ({ positions, indices: [0, 1, 2] })));
  expect(16 * 65535).toBeGreaterThan(1_000_000);
  expect(parsePreviewGlb(encode(repeated.json, repeated.binary))).toEqual(
    meshFromPositions(new Float32Array(Array.from({ length: 16 }, () => first).flat())),
  );
});

test('legacy GLB retains the one million position cap and rejects invalid u32 indices', () => {
  const fixture = wire([{ positions: first, indices: [0, 1, 2] }], 5125);
  fixture.binary.writeUInt32LE(3, 36);
  rejects(encode(fixture.json, fixture.binary));
  fixture.binary.writeUInt32LE(0, 36);
  fixture.json.accessors[0].count = 1_000_001;
  rejects(encode(fixture.json, fixture.binary));
});
