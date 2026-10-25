import { meshFromPositions, type StlMesh } from './stlParser';

/** Only the self-contained, single-mesh GLB emitted by rapidmaker-geometry is accepted.
 * This is deliberately not a general glTF loader; no URL, texture or external buffer is fetched.
 * Our writer stores millimetres plus a fixed node transform to metres/Y-up. Normalisation
 * below applies that same Z-up → Y-up rotation through meshFromPositions, before display.
 */
export function parsePreviewGlb(buffer: ArrayBuffer): StlMesh {
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
    start + binaryLength !== buffer.byteLength
  )
    fail();
  const data = JSON.parse(
    new TextDecoder('utf-8', { fatal: true }).decode(new Uint8Array(buffer, 20, jsonLength)),
  );
  if (
    data.asset?.version !== '2.0' ||
    data.asset?.generator !== 'rapidmaker-geometry' ||
    data.buffers?.length !== 1 ||
    data.buffers[0].uri !== undefined ||
    data.buffers[0].byteLength !== binaryLength ||
    data.bufferViews?.length !== 2 ||
    data.accessors?.length !== 2 ||
    data.meshes?.length !== 1 ||
    data.meshes[0].primitives?.length !== 1 ||
    data.nodes?.length !== 1 ||
    data.scene !== 0 ||
    data.scenes?.length !== 1 ||
    JSON.stringify(data.scenes[0].nodes) !== '[0]' ||
    data.extensionsRequired?.length
  )
    fail();
  const node = data.nodes[0],
    primitive = data.meshes[0].primitives[0];
  if (
    node.mesh !== 0 ||
    node.matrix ||
    node.translation ||
    node.children ||
    JSON.stringify(node.scale) !== '[0.001,0.001,0.001]' ||
    JSON.stringify(node.rotation) !== '[-0.70710678,0,0,0.70710678]' ||
    primitive.attributes?.POSITION !== 0 ||
    primitive.indices !== 1 ||
    (primitive.mode !== undefined && primitive.mode !== 4)
  )
    fail();
  const [positions, indices] = data.accessors;
  const [positionView, indexView] = data.bufferViews;
  const integer = (value: number) => Number.isSafeInteger(value) && value >= 0;
  if (
    positions.bufferView !== 0 ||
    positions.type !== 'VEC3' ||
    positions.componentType !== 5126 ||
    indices.bufferView !== 1 ||
    indices.type !== 'SCALAR' ||
    indices.componentType !== 5125 ||
    !integer(positions.count) ||
    !positions.count ||
    !integer(indices.count) ||
    !indices.count ||
    indices.count % 3 ||
    indices.count > 3_000_000 ||
    positions.count > 1_000_000 ||
    positions.sparse ||
    indices.sparse ||
    positions.normalized ||
    indices.normalized ||
    (positions.byteOffset ?? 0) !== 0 ||
    (indices.byteOffset ?? 0) !== 0 ||
    positionView.buffer !== 0 ||
    indexView.buffer !== 0 ||
    positionView.byteStride ||
    indexView.byteStride ||
    positionView.byteOffset !== 0 ||
    positionView.byteLength !== positions.count * 12 ||
    indexView.byteOffset !== positionView.byteLength ||
    indexView.byteLength !== indices.count * 4 ||
    positionView.byteLength + indexView.byteLength !== binaryLength
  )
    fail();
  const triangles = new Float32Array(indices.count * 3);
  for (let i = 0; i < indices.count; i++) {
    const index = view.getUint32(start + indexView.byteOffset + i * 4, true);
    if (index >= positions.count) fail();
    for (let axis = 0; axis < 3; axis++)
      triangles[i * 3 + axis] = view.getFloat32(start + index * 12 + axis * 4, true);
  }
  return meshFromPositions(triangles);
}
