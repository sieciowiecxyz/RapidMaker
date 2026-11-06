import { MAX_GLB_PREVIEW_TRIANGLES, meshFromPositions, type StlMesh } from './stlParser';

/** Only the self-contained, single-mesh GLB emitted by rapidmaker-geometry is accepted.
 * This is deliberately not a general glTF loader; no URL, texture or external buffer is fetched.
 * Our writer stores millimetres plus a fixed node transform to metres/Y-up. Normalisation
 * below applies that same Z-up → Y-up rotation through meshFromPositions, before display.
 */
export function parsePreviewGlb(buffer: ArrayBuffer): StlMesh {
  try {
    return parseCanonicalGlb(buffer);
  } catch {
    throw new Error('Nieprawidłowy format prywatnego podglądu GLB.');
  }
}

function parseCanonicalGlb(buffer: ArrayBuffer): StlMesh {
  const fail = () => {
    throw new Error('Nieprawidłowy format prywatnego podglądu GLB.');
  };
  if (buffer.byteLength < 32 || buffer.byteLength > 25 * 1024 * 1024) fail();
  const view = new DataView(buffer);
  if (
    view.getUint32(0, true) !== 0x46546c67 ||
    view.getUint32(4, true) !== 2 ||
    view.getUint32(8, true) !== buffer.byteLength ||
    view.getUint32(16, true) !== 0x4e4f534a
  )
    fail();
  const jsonLength = view.getUint32(12, true),
    binaryHeader = 20 + jsonLength;
  if (
    !jsonLength ||
    jsonLength > 64 * 1024 ||
    jsonLength % 4 ||
    binaryHeader + 8 > buffer.byteLength
  )
    fail();
  const binaryLength = view.getUint32(binaryHeader, true),
    start = binaryHeader + 8;
  if (
    view.getUint32(binaryHeader + 4, true) !== 0x004e4942 ||
    binaryLength % 4 ||
    start + binaryLength !== buffer.byteLength
  )
    fail();
  const data = JSON.parse(
    new TextDecoder('utf-8', { fatal: true }).decode(new Uint8Array(buffer, 20, jsonLength)),
  );
  const object = (value: unknown) =>
    value !== null && typeof value === 'object' && !Array.isArray(value);
  if (
    !object(data) ||
    data.asset?.version !== '2.0' ||
    data.asset?.generator !== 'rapidmaker-geometry' ||
    !Array.isArray(data.buffers) ||
    data.buffers.length !== 1 ||
    !object(data.buffers[0]) ||
    data.buffers[0].uri !== undefined ||
    data.buffers[0].byteLength !== binaryLength ||
    !Array.isArray(data.meshes) ||
    data.meshes.length !== 1 ||
    !object(data.meshes[0]) ||
    !Array.isArray(data.nodes) ||
    data.nodes.length !== 1 ||
    !object(data.nodes[0]) ||
    data.scene !== 0 ||
    !Array.isArray(data.scenes) ||
    data.scenes.length !== 1 ||
    !object(data.scenes[0]) ||
    JSON.stringify(data.scenes[0].nodes) !== '[0]' ||
    (data.extensionsRequired !== undefined &&
      (!Array.isArray(data.extensionsRequired) || data.extensionsRequired.length !== 0)) ||
    [
      'images',
      'textures',
      'materials',
      'samplers',
      'animations',
      'skins',
      'cameras',
      'extensions',
    ].some((key) => data[key] !== undefined)
  )
    fail();
  const node = data.nodes[0],
    primitives = data.meshes[0].primitives;
  if (
    node.mesh !== 0 ||
    node.matrix !== undefined ||
    node.translation !== undefined ||
    node.children !== undefined ||
    node.extensions !== undefined ||
    JSON.stringify(node.scale) !== '[0.001,0.001,0.001]' ||
    JSON.stringify(node.rotation) !== '[-0.70710678,0,0,0.70710678]' ||
    !Array.isArray(primitives) ||
    !primitives.length ||
    primitives.length > 64 ||
    !Array.isArray(data.accessors) ||
    data.accessors.length !== primitives.length * 2 ||
    !Array.isArray(data.bufferViews) ||
    data.bufferViews.length !== primitives.length * 2
  )
    fail();
  const integer = (value: number) => Number.isSafeInteger(value) && value >= 0;
  let totalIndices = 0,
    totalPositions = 0,
    offset = 0;
  const chunks: {
    count: number;
    positionCount: number;
    positionOffset: number;
    indexOffset: number;
    indexBytes: number;
  }[] = [];
  for (let chunk = 0; chunk < primitives.length; chunk++) {
    const p = 2 * chunk,
      i = p + 1;
    const primitive = primitives[chunk],
      positions = data.accessors[p],
      indices = data.accessors[i];
    const positionView = data.bufferViews[p],
      indexView = data.bufferViews[i];
    if (![primitive, positions, indices, positionView, indexView].every(object)) fail();
    const compact = indices.componentType === 5123,
      indexBytes = compact ? 2 : 4;
    if (
      !object(primitive.attributes) ||
      Object.keys(primitive.attributes).length !== 1 ||
      primitive.attributes.POSITION !== p ||
      primitive.indices !== i ||
      (primitive.mode !== undefined && primitive.mode !== 4) ||
      primitive.targets !== undefined ||
      primitive.extensions !== undefined ||
      primitive.material !== undefined ||
      positions.bufferView !== p ||
      positions.type !== 'VEC3' ||
      positions.componentType !== 5126 ||
      indices.bufferView !== i ||
      indices.type !== 'SCALAR' ||
      (!compact && (indices.componentType !== 5125 || primitives.length !== 1)) ||
      !integer(positions.count) ||
      !positions.count ||
      positions.count > (compact ? 65535 : 1_000_000) ||
      !integer(indices.count) ||
      !indices.count ||
      indices.count % 3 ||
      positions.sparse !== undefined ||
      indices.sparse !== undefined ||
      (positions.normalized !== undefined && positions.normalized !== false) ||
      (indices.normalized !== undefined && indices.normalized !== false) ||
      (positions.byteOffset !== undefined && positions.byteOffset !== 0) ||
      (indices.byteOffset !== undefined && indices.byteOffset !== 0) ||
      positions.extensions !== undefined ||
      indices.extensions !== undefined ||
      positionView.buffer !== 0 ||
      indexView.buffer !== 0 ||
      positionView.byteStride !== undefined ||
      indexView.byteStride !== undefined ||
      positionView.byteOffset !== offset ||
      positionView.byteLength !== positions.count * 12 ||
      indexView.byteOffset !== offset + positionView.byteLength ||
      indexView.byteLength !== indices.count * indexBytes
    )
      fail();
    totalIndices += indices.count;
    totalPositions += positions.count;
    if (
      totalIndices > 3 * MAX_GLB_PREVIEW_TRIANGLES ||
      totalPositions > 3 * MAX_GLB_PREVIEW_TRIANGLES
    )
      fail();
    const end = indexView.byteOffset + indexView.byteLength;
    offset = Math.ceil(end / 4) * 4;
    if (!integer(end) || offset > binaryLength) fail();
    for (let padding = end; padding < offset; padding++)
      if (view.getUint8(start + padding) !== 0) fail();
    chunks.push({
      count: indices.count,
      positionCount: positions.count,
      positionOffset: positionView.byteOffset,
      indexOffset: indexView.byteOffset,
      indexBytes,
    });
  }
  if (offset !== binaryLength) fail();
  // Only flattened triangles are allocated; chunk-local positions stay in the bounded input.
  const triangles = new Float32Array(totalIndices * 3);
  let target = 0;
  for (const chunk of chunks) {
    for (let i = 0; i < chunk.count; i++) {
      const address = start + chunk.indexOffset + i * chunk.indexBytes;
      const index =
        chunk.indexBytes === 2 ? view.getUint16(address, true) : view.getUint32(address, true);
      if (index >= chunk.positionCount || (chunk.indexBytes === 2 && index === 65535)) fail();
      for (let axis = 0; axis < 3; axis++)
        triangles[target++] = view.getFloat32(
          start + chunk.positionOffset + index * 12 + axis * 4,
          true,
        );
    }
  }
  return meshFromPositions(triangles, MAX_GLB_PREVIEW_TRIANGLES);
}
