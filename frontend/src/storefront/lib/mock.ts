import type { Model } from './types';
export async function getSampleModels(count = 1): Promise<Model[]> {
  if (count === 1) return [{ id: 'sample-bracket', name: 'uchwyt-montazowy.stl', quantity: 2, sample: true }];
  const names = ['korpus-obudowy.stl', 'pokrywa-obudowy.stl', 'uchwyt-montazowy-lewy.stl', 'uchwyt-montazowy-prawy.stl', 'zebatka-napedowa.stl', 'adapter-czujnika.stl', 'oslona-przewodow.stl', 'wspornik-silnika.stl', 'dystans-montazowy.stl', 'zatrzask-pokrywy.stl'];
  return names.slice(0, count).map((name, i) => ({ id: `sample-batch-${i}`, name, quantity: 1, sample: true }));
}
