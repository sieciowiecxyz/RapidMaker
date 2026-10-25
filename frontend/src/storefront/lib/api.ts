import * as checkout from '../../lib/api';
import type { DeliveryMethod, PrintModel, PrintOptionsCatalog } from '../../lib/types';
import { getSampleModels as samples } from './mock';
import { validateModel } from './modelSettings';
import type { Material, Model, Quote, QuoteOptions } from './types';

export async function getMaterials(): Promise<Material[]> {
  const colors = [
    { id: 'black', name: 'Czarny', hex: '#292e2a', available: true },
    { id: 'white', name: 'Biały', hex: '#f2f0e8', available: true },
    { id: 'green', name: 'Zielony', hex: '#427756', available: true },
    { id: 'custom', name: 'Niestandardowy', hex: '#a7b5a2', available: true },
  ];
  return [
    {id:'pla',family:'PLA',name:'PLA',description:'Modele poglądowe i prototypy. Dobry wybór na początek.',colors},
    {id:'petg',family:'PETG',name:'PETG',description:'Części użytkowe i obudowy.',colors},
    {id:'asa',family:'ASA',name:'ASA',description:'Elementy do zastosowań zewnętrznych.',colors},
    {id:'pa',family:'PA-CF',name:'PA-CF',description:'Poliamid wzmacniany włóknem węglowym do elementów konstrukcyjnych.',colors:colors.filter(c=>c.id==='black')},
    {id:'tpu',family:'TPU',name:'TPU',description:'Elastyczne elementy, osłony i amortyzacja.',colors},
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
    units: 'mm',
    options: {
      materialId: settings.material,
      color: color.id,
      quantity: model.quantity,
      quality: settings.quality as 'standard' | 'fine',
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
  // Configuration is local; the production server must confirm a price separately.
  for (const model of options.models) {
    const issues = validateModel(model, materials);
    if (issues.length) throw new Error(`${model.name}: uzupełnij ${issues.join(', ')}.`);
  }
  if (!options.models.length || options.models.length > 10) throw new Error('Dodaj od 1 do 10 części.');
  return {lines:[],total:0,setup:0,delivery:0,manual:true};
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
    (['courier', 'locker', 'pickup'].includes(delivery) ? delivery : 'courier') as DeliveryMethod,
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
    delivery: cart.delivery,
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
        quality: model.options.quality === 'fine' ? 'fine' : 'standard',
        strength:
          model.strength === 'strong'
            ? 'reinforced'
            : model.strength === 'light'
              ? 'light'
              : 'standard',
      },
      checkoutOptions: { advanced: model.advanced, notes: model.notes, strength: model.strength },
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
