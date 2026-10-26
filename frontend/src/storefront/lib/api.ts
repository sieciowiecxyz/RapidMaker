import * as checkout from '../../lib/api';
import type { DeliveryMethod, PrintModel, PrintOptionsCatalog } from '../../lib/types';
import { getSampleModels as samples } from './mock';
import { validateModel } from './modelSettings';
import type { Material, Model, Quote, QuoteOptions } from './types';

export async function getMaterials(): Promise<Material[]> {
  const catalog = await checkout.getMaterials();
  const families = [
    {
      id: 'GFA00',
      family: 'PLA' as const,
      description: 'Prototypy, dekoracje i codzienne części.',
    },
    {
      id: 'GFB00',
      family: 'ABS' as const,
      description: 'Sztywne części techniczne, odporniejsze na temperaturę.',
    },
    {
      id: 'GFU01',
      family: 'TPU' as const,
      description: 'Elastyczne elementy, osłony i amortyzacja.',
    },
  ];
  return families.flatMap(({ id, family, description }) => {
    const material = catalog.find((item) => item.id === id);
    return material
      ? [
          {
            id,
            family,
            name: family,
            description,
            compatible: material.compatible,
            colors: material.colors.map((color) => ({
              id: color.code ?? color.hex,
              name: color.name,
              hex: color.hex,
              available: true,
            })),
          },
        ]
      : [];
  });
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

function productionModel(
  model: Model,
  materials: Material[],
  catalog: PrintOptionsCatalog,
): PrintModel {
  const settings = model.settings!;
  const material = materials.find((item) => item.id === settings.material)!;
  const color = material.colors.find((item) => item.id === settings.color)!;
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
      materialId: settings.material,
      color: color.id,
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
    analysis: model.analysis,
    dimensions: model.analysis
      ? model.analysis.dimensions_mm.map((value) => Math.round(value * 10) / 10).join(' × ') + ' mm'
      : 'Wymaga analizy',
    weight: 'Masę obliczy slicer',
    notes: previous?.notes ?? '',
  };
}
export async function getQuote(options: QuoteOptions): Promise<Quote> {
  const materials = await getMaterials();
  validate(options.models, materials);
  const catalog = await checkout.getPrintOptions();
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
): Promise<void> {
  const materials = await getMaterials();
  validate(models, materials);
  const catalog = await checkout.getPrintOptions();
  const oldCart = await checkout.getCart();
  for (const item of oldCart.models) await checkout.removeModel(item.id);
  for (const model of models) {
    const cart = await checkout.addModel(model.name);
    const added = cart.models[cart.models.length - 1];
    const draft = productionModel(model, materials, catalog);
    if (manual)
      draft.notes = [draft.notes, 'Indywidualne wymagania — proszę o kontakt przed realizacją.']
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
  const materials = await getMaterials();
  return {
    delivery: cart.delivery === 'pickup' ? 'pickup' : 'shipping',
    models: cart.models.map((model) => ({
      id: model.id,
      name: model.name,
      quantity: model.options.quantity,
      sample: !model.analysis,
      analysis: model.analysis,
      status: model.analysis ? 'ready' : undefined,
      settings: {
        material: model.options.materialId,
        color:
          materials
            .find((item) => item.id === model.options.materialId)
            ?.colors.find(
              (color) => color.id === model.options.color || color.name === model.options.color,
            )?.id ?? '',
        customShade: '',
        quality: model.options.quality,
        strength:
          model.strength === 'strong'
            ? 'reinforced'
            : model.strength === 'light'
              ? 'light'
              : 'standard',
      },
      checkoutOptions: {
        advanced: model.advanced,
        notes: model.notes,
        strength: model.strength,
        units: model.units,
      },
    })),
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
