import * as checkout from '../../lib/api';
import type { DeliveryMethod, PrintModel, PrintOptionsCatalog } from '../../lib/types';
import { getSampleModels as samples } from './mock';
import { validateModel } from './modelSettings';
import { customColors } from './colors';
import type { Material, Model, Quote, QuoteOptions } from './types';

export async function getMaterials(): Promise<Material[]> {
  const colors = [
    { id: 'black', name: 'Czarny', hex: '#292e2a', available: true },
    { id: 'white', name: 'Biały', hex: '#f2f0e8', available: true },
    { id: 'green', name: 'Zielony', hex: '#427756', available: true },
    { id: 'custom', name: 'Niestandardowy', hex: '#a7b5a2', available: true },
  ];
  return [
    {
      id: 'pla',
      family: 'PLA',
      name: 'PLA',
      description: 'Modele poglądowe i prototypy. Dobry wybór na początek.',
      colors,
    },
    { id: 'petg', family: 'PETG', name: 'PETG', description: 'Części użytkowe i obudowy.', colors },
    {
      id: 'asa',
      family: 'ASA',
      name: 'ASA',
      description: 'Elementy do zastosowań zewnętrznych.',
      colors,
    },
    {
      id: 'pa',
      family: 'PA-CF',
      name: 'PA-CF',
      description: 'Poliamid wzmacniany włóknem węglowym do elementów konstrukcyjnych.',
      colors: colors.filter((c) => c.id === 'black'),
    },
    {
      id: 'tpu',
      family: 'TPU',
      name: 'TPU',
      description: 'Elastyczne elementy, osłony i amortyzacja.',
      colors,
    },
  ];
}

export const getSampleModels = samples;
export const analyzeFile = checkout.analyzeModel;

function validate(models: Model[], materials: Material[]) {
  if (!models.length || models.length > 10) throw new Error('Dodaj od 1 do 10 części.');
  for (const model of models) {
    const issues = validateModel(model, materials);
    if (issues.length) throw new Error(`${model.name}: uzupełnij ${issues.join(', ')}.`);
    if (model.status === 'analyzing') throw new Error('Poczekaj na analizę wszystkich plików.');
    if (model.status === 'error') throw new Error(model.error ?? 'Popraw plik modelu.');
  }
}

const profiles: Record<string, string> = {
  pla: 'GFA00',
  petg: 'GFG02',
  asa: 'GFB01',
  pa: 'GFN03',
  tpu: 'GFU01',
};
const defaults: Record<string, Record<string, string>> = {
  GFA00: { black: '10101', white: '10100', green: '10501' },
  GFG02: { black: '33102', white: '33100', green: '33500' },
  GFB01: { black: '45101', white: '45100', green: '45500' },
  GFU01: { black: '50101', white: '50100' },
  GFN03: { black: 'black' },
};
type ProductionMaterial = Awaited<ReturnType<typeof checkout.getMaterials>>[number];
function selection(model: Model, catalog: ProductionMaterial[]) {
  const settings = model.settings!;
  const shade =
    settings.color === 'custom'
      ? customColors.find((c) => c.id === settings.customShade)
      : undefined;
  const series = settings.material === 'pla' && shade?.id.match(/^bambu-(GFA\d{2})-/)?.[1];
  const id = series || profiles[settings.material] || settings.material;
  const material = catalog.find((item) => item.id === id);
  if (!material) throw new Error('Brak konfiguracji wybranego materiału. Spróbuj później.');
  const code = shade && series ? shade.code : !shade ? defaults[id]?.[settings.color] : undefined;
  const color = code ? material.colors.find((item) => item.code === code) : undefined;
  const colorPreference = color
    ? undefined
    : shade
      ? `${shade.name} · ${shade.series}${shade.code ? ' · ' + shade.code : ''} (${shade.hexes.join(', ')})`
      : ({ black: 'Czarny', white: 'Biały', green: 'Zielony' } as Record<string, string>)[
          settings.color
        ];
  if (!color && !colorPreference) throw new Error('Wybierz kolor modelu.');
  return { material, color: color?.code ?? '', colorPreference };
}
function productionModel(
  model: Model,
  materials: ProductionMaterial[],
  catalog: PrintOptionsCatalog,
): PrintModel {
  const settings = model.settings!;
  const resolved = selection(model, materials);
  const strength =
    settings.strength === 'reinforced' ? 'strong' : (settings.strength as 'light' | 'standard');
  const previous = model.checkoutOptions;
  const selected =
    previous?.strength === 'strong'
      ? 'strong'
      : previous?.strength === 'custom'
        ? 'standard'
        : previous?.strength;
  const preset = catalog.strengths.find((item) => item.value === strength)!;
  return {
    id: model.id,
    name: model.name,
    format: /\.(step|stp)$/i.test(model.name) ? 'STEP' : /\.3mf$/i.test(model.name) ? '3MF' : 'STL',
    parts: 1,
    units: previous?.units ?? 'mm',
    options: {
      materialId: resolved.material.id,
      color: resolved.color,
      quantity: model.quantity,
      quality: settings.quality as 'draft' | 'standard' | 'fine',
    },
    strength: selected === strength ? previous!.strength : strength,
    advanced: {
      wallCount: preset.wallCount,
      infill: preset.infill,
      pattern: 'grid',
      orientation: 'auto',
      supports: 'auto',
      finish: 'standard',
      fit: 'normal',
      insertCount: 0,
      insertSize: 'M3',
      turnaround: 'standard',
      drawing: null,
      ...previous?.advanced,
      ...(selected === strength ? {} : { wallCount: preset.wallCount, infill: preset.infill }),
    },
    colorPreference: resolved.colorPreference,
    analysis: model.analysis,
    dimensions: model.analysis
      ? model.analysis.dimensions_mm.map((value) => Math.round(value * 10) / 10).join(' × ') + ' mm'
      : 'Wymaga analizy',
    weight: 'Masę obliczy slicer',
    notes: previous?.notes ?? '',
  };
}
export async function getQuote(options: QuoteOptions): Promise<Quote> {
  validate(options.models, await getMaterials());
  const [materials, catalog] = await Promise.all([
    checkout.getMaterials(),
    checkout.getPrintOptions(),
  ]);
  const models = options.models.map((model) => productionModel(model, materials, catalog));
  const result = await checkout.quoteModels(models, 'pickup', undefined, options.manual);
  const quote = result.checkout;
  return {
    lines: quote.print_lines.map((line) => ({
      id: line.model_id,
      unitPrice: line.unit_grosze / 100,
      subtotal: line.total_grosze / 100,
    })),
    total: quote.print_grosze / 100,
    setup: quote.setup_grosze / 100,
    delivery: 0,
    manual: result.manual || quote.packing.status === 'manual_review',
    reason: quote.reason ?? quote.packing.reason,
  };
}
export async function prepareCheckout(
  models: Model[],
  delivery: string,
  manual = false,
  notes = '',
): Promise<void> {
  validate(models, await getMaterials());
  const [materials, catalog] = await Promise.all([
    checkout.getMaterials(),
    checkout.getPrintOptions(),
  ]);
  const resolved = models.map((model) => productionModel(model, materials, catalog));
  const oldCart = await checkout.getCart();
  for (const item of oldCart.models) await checkout.removeModel(item.id);
  for (const draft of resolved) {
    const cart = await checkout.addModel(draft.name);
    const added = cart.models[cart.models.length - 1];
    if (manual || notes.trim())
      draft.notes = [
        draft.notes,
        notes.trim(),
        ...(manual ? ['Indywidualne wymagania — proszę o kontakt przed realizacją.'] : []),
      ]
        .filter(Boolean)
        .join(' ');
    await checkout.updateModel(added.id, { ...draft, id: added.id });
  }
  await checkout.setDelivery(
    (['courier', 'locker', 'pickup'].includes(delivery)
      ? delivery
      : oldCart.delivery === 'locker'
        ? 'locker'
        : 'courier') as DeliveryMethod,
    oldCart.postalCode,
  );
  saveDraft(models, delivery);
  try {
    localStorage.setItem(CHECKOUT_READY_KEY, '1');
  } catch {
    /* Storage is optional. */
  }
}

const CHECKOUT_READY_KEY = 'rapidmaker.storefront.checkout-ready';
export async function restoreCheckoutDraft(): Promise<{
  models: Model[];
  delivery: string;
} | null> {
  try {
    if (localStorage.getItem(CHECKOUT_READY_KEY) !== '1') return null;
  } catch {
    return null;
  }
  const cart = await checkout.getCart();
  if (!cart.models.length) return null;
  const saved = loadDraft()?.models ?? [];
  return {
    delivery: cart.delivery === 'pickup' ? 'pickup' : 'shipping',
    models: cart.models.map((model) => {
      const family = Object.keys(profiles).find(key => profiles[key] === model.options.materialId)
        ?? (model.options.materialId.startsWith('GFA') ? 'pla' : '');
      const shade = customColors.find(c => c.id === `bambu-${model.options.materialId}-${model.options.color}`);
      const basic = Object.entries(defaults[model.options.materialId] ?? {}).find(([, code]) => code === model.options.color)?.[0];
      const previous = saved.find(item => item.analysis?.id === model.analysis?.id && item.name === model.name)?.settings;
      return {
        id:model.id, name:model.name, quantity:model.options.quantity,
        sample:!model.analysis, analysis:model.analysis, status:model.analysis ? 'ready' as const : undefined,
        settings:{
          material:family, color:basic ?? 'custom', customShade:shade?.id ?? '', ...previous,
          quality:model.options.quality,
          strength:model.strength === 'strong' ? 'reinforced' : model.strength === 'light' ? 'light' : 'standard',
        },
        checkoutOptions:{advanced:model.advanced,notes:model.notes,strength:model.strength,units:model.units},
      };
    }),
  };
}

const DRAFT_KEY = 'rapidmaker.storefront.draft.v1';
export function saveDraft(models: Model[], delivery: string): void {
  try {
    localStorage.setItem(DRAFT_KEY, JSON.stringify({ models, delivery }));
  } catch {
    /* Storage is optional. */
  }
}
export function loadDraft(): { models: Model[]; delivery: string } | null {
  try {
    const draft = JSON.parse(localStorage.getItem(DRAFT_KEY) ?? 'null');
    if (
      !draft ||
      !Array.isArray(draft.models) ||
      draft.models.length > 10 ||
      typeof draft.delivery !== 'string'
    )
      return null;
    if (
      !draft.models.every(
        (model: Model) =>
          typeof model?.id === 'string' &&
          typeof model.name === 'string' &&
          Number.isInteger(model.quantity),
      )
    )
      return null;
    return draft;
  } catch {
    return null;
  }
}
