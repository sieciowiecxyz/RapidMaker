import * as checkout from '../../lib/api';
import type { CheckoutQuote, DeliveryMethod, PrintModel } from '../../lib/types';
import { getSampleModels as samples } from './mock';
import { validateModel } from './modelSettings';
import type { Material, Model, Quote, QuoteOptions } from './types';

export async function getMaterials(): Promise<Material[]> {
  return (await checkout.getMaterials()).map(material => ({
    id: material.id,
    name: material.name,
    description: material.description,
    compatible: material.compatible,
    colors: material.colors.map(color => ({
      id: color.hex,
      name: color.name,
      hex: color.hex,
      available: material.compatible && color.inStock,
    })),
  }));
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

function needsReview(model: Model): boolean {
  const saved = model.checkoutOptions;
  if (!saved) return false;
  const a = saved.advanced;
  return !!saved.notes.trim() || a.orientation !== 'auto' || a.supports !== 'auto' || a.finish !== 'standard' || a.fit !== 'normal' || a.insertCount > 0 || a.turnaround !== 'standard' || !!a.drawing;
}

export async function getQuote(options: QuoteOptions): Promise<Quote> {
  const materials = await getMaterials();
  validate(options.models, materials);
  const manual = options.manual || options.models.some(model => !model.analysis || needsReview(model)) || options.models.reduce((count, model) => count + model.quantity, 0) > 50;
  const response = await fetch('/api/checkout/quote', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      parts: options.models.map(model => ({
        id: model.id,
        quantity: model.quantity,
        analysis_id: model.analysis?.id,
      })),
      delivery: 'pickup',
      manual,
    }),
  });
  const data = await response.json();
  if (!response.ok) throw new Error(data.message ?? 'Nie udało się pobrać wyceny.');
  const quote = data as CheckoutQuote;
  return {
    lines: quote.print_lines.map(line => ({
      id: line.model_id,
      unitPrice: line.unit_grosze / 100,
      subtotal: line.total_grosze / 100,
    })),
    total: quote.print_grosze / 100,
    setup: quote.setup_grosze / 100,
    delivery: 0,
    manual: manual || quote.packing.status === 'manual_review',
    reason: quote.reason ?? quote.packing.reason,
  };
}

export async function prepareCheckout(models: Model[], delivery: string, manual = false): Promise<void> {
  const materials = await getMaterials();
  validate(models, materials);
  const catalog = await checkout.getPrintOptions();
  const oldCart = await checkout.getCart();
  for (const item of oldCart.models) await checkout.removeModel(item.id);
  for (const model of models) {
    const settings = model.settings!;
    const material = materials.find(item => item.id === settings.material)!;
    const color = material.colors.find(item => item.id === settings.color)!;
    const selectedStrength: PrintModel['strength'] = settings.strength === 'reinforced' ? 'strong' : settings.strength as 'light' | 'standard';
    const previous = model.checkoutOptions;
    const previousSelection = previous?.strength === 'strong' ? 'strong' : previous?.strength === 'custom' ? 'standard' : previous?.strength;
    const strength = previousSelection === selectedStrength ? previous!.strength : selectedStrength;
    const preset = catalog.strengths.find(item => item.value === selectedStrength)!;
    const cart = await checkout.addModel(model.name);
    const added = cart.models[cart.models.length - 1];
    await checkout.updateModel(added.id, { strength });
    await checkout.updateModel(added.id, {
      options: {
        materialId: settings.material,
        color: color.name,
        quantity: model.quantity,
        quality: settings.quality as 'standard' | 'fine',
      },
      strength,
      advanced: { ...added.advanced, ...previous?.advanced, ...(previousSelection === selectedStrength ? {} : { wallCount: preset.wallCount, infill: preset.infill }) },
      analysis: model.analysis,
      shipping: undefined,
      dimensions: model.analysis ? model.analysis.dimensions_mm.map(value => Math.round(value * 10) / 10).join(' × ') + ' mm' : 'Wymaga analizy',
      weight: model.analysis?.weight_g != null ? `${Math.ceil(model.analysis.weight_g)} g (szacunek)` : 'Wymaga analizy',
      notes: [...new Set([...(previous?.notes ? [previous.notes] : []), manual ? 'Indywidualne wymagania — proszę o kontakt przed realizacją.' : '', model.analysis ? '' : 'Brak analizy geometrii — wymagana indywidualna wycena przed płatnością.'].filter(Boolean))].join(' '),
    });
  }
  await checkout.setDelivery((['courier', 'locker', 'pickup'].includes(delivery) ? delivery : 'courier') as DeliveryMethod, oldCart.postalCode);
  saveDraft(models, delivery);
  try { localStorage.setItem(CHECKOUT_READY_KEY, '1'); } catch { /* Storage is optional. */ }
}

const CHECKOUT_READY_KEY = 'rapidmaker.storefront.checkout-ready';
export async function restoreCheckoutDraft(): Promise<{ models: Model[]; delivery: string } | null> {
  try { if (localStorage.getItem(CHECKOUT_READY_KEY) !== '1') return null; } catch { return null; }
  const cart = await checkout.getCart();
  if (!cart.models.length) return null;
  const materials = await getMaterials();
  return {
    delivery: cart.delivery,
    models: cart.models.map(model => ({
      id: model.id,
      name: model.name,
      quantity: model.options.quantity,
      sample: !model.analysis,
      analysis: model.analysis,
      status: model.analysis ? 'ready' : undefined,
      settings: {
        material: model.options.materialId,
        color: materials.find(item => item.id === model.options.materialId)?.colors.find(color => color.name === model.options.color)?.id ?? '',
        customShade: '',
        quality: model.options.quality === 'fine' ? 'fine' : 'standard',
        strength: model.strength === 'strong' ? 'reinforced' : model.strength === 'light' ? 'light' : 'standard',
      },
      checkoutOptions: { advanced: model.advanced, notes: model.notes, strength: model.strength },
    })),
  };
}

const DRAFT_KEY = 'rapidmaker.storefront.draft.v1';
export function saveDraft(models: Model[], delivery: string): void {
  try { localStorage.setItem(DRAFT_KEY, JSON.stringify({ models, delivery })); } catch { /* Storage is optional. */ }
}
export function loadDraft(): { models: Model[]; delivery: string } | null {
  try {
    const draft = JSON.parse(localStorage.getItem(DRAFT_KEY) ?? 'null');
    if (!draft || !Array.isArray(draft.models) || draft.models.length > 10 || typeof draft.delivery !== 'string') return null;
    if (!draft.models.every((model: Model) => typeof model?.id === 'string' && typeof model.name === 'string' && Number.isInteger(model.quantity))) return null;
    return draft;
  } catch { return null; }
}
