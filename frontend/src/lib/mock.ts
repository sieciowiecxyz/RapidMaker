import { validateModel } from './modelSettings';
import type { Material, Model, Quote, QuoteOptions } from './types';
const colors = [
  { id: 'black', name: 'Czarny', hex: '#292e2a', available: true },
  { id: 'white', name: 'Biały', hex: '#f2f0e8', available: true },
  { id: 'green', name: 'Zielony', hex: '#427756', available: true },
  { id: 'custom', name: 'Niestandardowy', hex: '#a7b5a2', available: true }
];
const materials: Material[] = [
  { id: 'pla', name: 'PLA', description: 'Modele poglądowe i prototypy. Dobry wybór na początek.', colors },
  { id: 'petg', name: 'PETG', description: 'Części użytkowe i obudowy. Dobór materiału potwierdzimy przy wycenie.', colors },
  { id: 'asa', name: 'ASA', description: 'Elementy do zastosowań zewnętrznych. Skonsultujemy warunki pracy części.', colors },
  { id: 'pa', name: 'PA-CF', description: 'Poliamid wzmacniany włóknem węglowym do elementów konstrukcyjnych. Obciążenia i warunki pracy wymagają indywidualnego doboru.', colors: colors.filter(color => color.id === 'black') },
  { id: 'tpu', name: 'TPU', description: 'Elastyczne elementy i osłony. Nietypową geometrię wycenimy indywidualnie.', colors }
];
export async function getMaterials(): Promise<Material[]> { return structuredClone(materials); }
export async function getSampleModels(count = 1): Promise<Model[]> {
  if (count === 1) return [{ id: 'sample-bracket', name: 'uchwyt-montazowy.stl', quantity: 2, sample: true }];
  const names = ['korpus-obudowy.stl', 'pokrywa-obudowy.stl', 'uchwyt-montazowy-lewy.stl', 'uchwyt-montazowy-prawy.stl', 'zebatka-napedowa.stl', 'adapter-czujnika.stl', 'oslona-przewodow.stl', 'wspornik-silnika.stl', 'dystans-montazowy.stl', 'zatrzask-pokrywy.stl'];
  return names.slice(0, count).map((name, i) => ({ id: `sample-batch-${i}`, name, quantity: 1, sample: true }));
}
export async function getQuote(options: QuoteOptions): Promise<Quote> {
  await new Promise(resolve => setTimeout(resolve, 650));
  if (options.simulateError) throw new Error('Nie udało się przeliczyć przykładu. Spróbuj ponownie.');
  if (!options.models.length) throw new Error('Dodaj model lub użyj przykładu.');
  if (options.models.length > 10) throw new Error('Dodaj maksymalnie 10 modeli.');
  const fixtures: Record<string, Record<string, number>> = { pla: { standard: 24, fine: 31 }, petg: { standard: 29, fine: 37 }, asa: { standard: 35, fine: 44 }, tpu: { standard: 39, fine: 49 }, pa: { standard: 45, fine: 56 } };
  const strengthMultiplier: Record<string, number> = { light: .9, standard: 1, reinforced: 1.25 };
  const lines = options.models.map(model => {
    const errors=validateModel(model,materials);
    if(errors.length) throw new Error(`${model.name}: uzupełnij ${errors.join(', ')}.`);
    const settings=model.settings!;
    // Synthetic fixture values, unrelated to file geometry or a production tariff.
    const unitPrice=Math.round(fixtures[settings.material][settings.quality]*strengthMultiplier[settings.strength]*100)/100;
    return {id:model.id,unitPrice,subtotal:Math.round(unitPrice*model.quantity*100)/100};
  });
  const delivery = options.delivery === 'pickup' ? 0 : 18;
  return { lines, delivery, total: lines.reduce((sum, line) => sum + line.subtotal, 0) + delivery, manual: options.manual || options.models.reduce((sum, model) => sum + model.quantity, 0) > 50 };
}
