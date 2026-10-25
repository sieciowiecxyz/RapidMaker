import type { PartMesh } from './partMesh';
export type StlMesh = PartMesh & { triangleCount: number; dimensions: [number, number, number] };
const MAX_TRIANGLES = 1_000_000;
export function parseStl(buffer: ArrayBuffer): StlMesh {
  if (!buffer.byteLength || buffer.byteLength > 25 * 1024 * 1024)
    throw new Error('Wybierz niepusty STL do 25 MB.');
  const view = new DataView(buffer);
  const count = buffer.byteLength >= 84 ? view.getUint32(80, true) : 0;
  const binary =
    count > 0 &&
    84 + count * 50 <= buffer.byteLength &&
    buffer.byteLength - (84 + count * 50) < 512;
  let positions: Float32Array;
  if (binary) {
    if (count > MAX_TRIANGLES)
      throw new Error('Model ma ponad 1 mln trójkątów. Wyeksportuj lżejszy STL do podglądu.');
    positions = new Float32Array(count * 9);
    for (let i = 0; i < count; i++)
      for (let j = 0; j < 9; j++)
        positions[i * 9 + j] = view.getFloat32(84 + i * 50 + 12 + j * 4, true);
  } else {
    const text = new TextDecoder().decode(buffer);
    if (!/^\s*solid(?:\s|$)/i.test(text))
      throw new Error('Nie udało się odczytać STL. Plik może być uszkodzony lub mieć inny format.');
    const number = '([+-]?(?:\\d+\\.?\\d*|\\.\\d+)(?:[eE][+-]?\\d+)?)';
    const regex = new RegExp(
      '\\bvertex\\s+' + number + '\\s+' + number + '\\s+' + number + '(?=\\s|$)',
      'gi',
    );
    const values: number[] = [];
    let match: RegExpExecArray | null;
    while ((match = regex.exec(text))) {
      values.push(Number(match[1]), Number(match[2]), Number(match[3]));
      if (values.length > MAX_TRIANGLES * 9)
        throw new Error('Model ma ponad 1 mln trójkątów. Wyeksportuj lżejszy STL do podglądu.');
    }
    if (!values.length || values.length % 9 !== 0)
      throw new Error('STL nie zawiera poprawnych trójkątów.');
    positions = new Float32Array(values);
  }
  return meshFromPositions(positions);
}

/** Flattened triangles in source millimetres; normalise once for the existing WebGL viewer. */
export function meshFromPositions(positions: Float32Array): StlMesh {
  if (!positions.length || positions.length % 9 || positions.length / 9 > MAX_TRIANGLES)
    throw new Error('Podgląd wymaga poprawnej siatki do 1 mln trójkątów.');
  const min = [Infinity, Infinity, Infinity],
    max = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < positions.length; i++) {
    const value = positions[i],
      axis = i % 3;
    if (!Number.isFinite(value)) throw new Error('STL zawiera nieprawidłowe współrzędne.');
    min[axis] = Math.min(min[axis], value);
    max[axis] = Math.max(max[axis], value);
  }
  const dimensions = max.map((value, i) => value - min[i]) as [number, number, number];
  const span = Math.max(...dimensions);
  if (!Number.isFinite(span) || span <= 0)
    throw new Error('Model nie ma geometrii możliwej do wyświetlenia.');
  const center = min.map((value, i) => value / 2 + max[i] / 2),
    scale = 180 / span;
  const vertices = new Float32Array(positions.length * 2);
  let offset = 0,
    minY = Infinity,
    maxY = -Infinity;
  for (let i = 0; i < positions.length; i += 9) {
    const points = Array.from({ length: 3 }, (_, j) => {
      const k = i + j * 3;
      return [
        (positions[k] - center[0]) * scale,
        (positions[k + 2] - center[2]) * scale,
        -(positions[k + 1] - center[1]) * scale,
      ];
    });
    const [a, b, c] = points,
      ux = b[0] - a[0],
      uy = b[1] - a[1],
      uz = b[2] - a[2],
      vx = c[0] - a[0],
      vy = c[1] - a[1],
      vz = c[2] - a[2];
    const normal = [uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx],
      length = Math.hypot(...normal);
    if (length < 1e-12) continue;
    for (const point of points) {
      vertices.set(point, offset);
      vertices.set(
        normal.map((value) => value / length),
        offset + 3,
      );
      offset += 6;
      minY = Math.min(minY, point[1]);
      maxY = Math.max(maxY, point[1]);
    }
  }
  if (!offset) throw new Error('STL zawiera wyłącznie trójkąty bez powierzchni.');
  return {
    vertices: offset === vertices.length ? vertices : vertices.slice(0, offset),
    minY,
    maxY,
    triangleCount: offset / 18,
    dimensions,
  };
}
