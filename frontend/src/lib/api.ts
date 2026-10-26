// Local drafts stay in the cart; accounts and orders use the server.
export {
  getPrintOptions,
  getDemoModel,
  getCheckoutOrder,
  getCart,
  updateModel,
  removeModel,
  setDelivery,
  getDeliveryOptions,
  retryPayment,
  updateMaterial,
  getSystemSettings,
  updateSystemSettings,
  retryJob,
  getDemoScenario,
  setDemoScenario,
  resetDemo,
} from './mock';

import { LEGAL_VERSION } from './legal';
import * as mock from './mock';
import { apiFetch, ApiError } from './http';
export { getSession, requestLogin, confirmLogin, logout, claimGuest } from './session';
export {
  getOrders,
  getOrder,
  reorder,
  sendMessage,
  markMessagesRead,
  getProfile,
  updateProfile,
  deleteAccount,
  updateOrder,
  sendOffer,
  refundOrder,
  createShippingLabel,
} from './orders';
import { getOrder } from './orders';
import type {
  CheckoutQuote,
  Contact,
  Quote,
  PickupPoint,
  StripeStatus,
  Order,
  Material,
  PrintModel,
  DeliveryMethod,
} from './types';
async function request<T>(path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const response = await apiFetch(path, {
    signal,
    method: body === undefined ? 'GET' : 'POST',
    headers: { 'Content-Type': 'application/json' },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  const data = await response.json();
  if (!response.ok)
    throw Object.assign(
      new ApiError(
        response.status,
        data.message ?? data.error ?? 'Usługa wyceny jest niedostępna.',
        data.code,
      ),
      { reason: data.reason, field_errors: data.field_errors },
    );
  return data as T;
}
export function quoteCart(contact?: Contact, signal?: AbortSignal): Promise<Quote> {
  return calculateQuote(contact, false, signal);
}

export function quotePrintCart(): Promise<Quote> {
  return calculateQuote(undefined, true);
}

type CatalogMaterial = {
  id: string;
  name: string;
  available: boolean;
  colors: { code: string; name: string; rgba: string[]; in_stock: boolean }[];
};
export async function getMaterials(): Promise<Material[]> {
  const result = await request<{ materials: CatalogMaterial[] }>('/api/catalog/materials');
  return result.materials.map((material) => ({
    id: material.id,
    name: material.name,
    description: material.available
      ? 'Dostępny do automatycznej wyceny.'
      : 'Dostępność i wycena wymagają potwierdzenia zespołu.',
    compatible: material.available,
    costPerKg: 0,
    priceMultiplier: 1,
    colors: material.colors.map((color) => ({
      code: color.code,
      name: color.name,
      hex: color.rgba[0]?.slice(0, 7) ?? '#808080',
      inStock: true,
    })),
  }));
}
export async function addModel(name: string) {
  const materials = await getMaterials();
  const material = materials.find((item) => item.compatible) ?? materials[0];
  const color = material?.colors.find((item) => item.inStock) ?? material?.colors[0];
  const cart = await mock.addModel(name);
  const added = cart.models[cart.models.length - 1];
  return mock.updateModel(added.id, {
    options: { ...added.options, materialId: material?.id ?? '', color: color?.name ?? '' },
    dimensions: 'Wymaga analizy',
    weight: 'Wymaga pomiaru slicerem',
  });
}

export function needsManualReview(
  model: Pick<PrintModel, 'notes' | 'advanced' | 'units'>,
): boolean {
  const a = model.advanced;
  return (
    model.units !== 'mm' ||
    !!model.notes.trim() ||
    a.pattern !== 'grid' ||
    a.orientation !== 'auto' ||
    a.supports !== 'auto' ||
    a.finish !== 'standard' ||
    a.fit !== 'normal' ||
    a.insertCount > 0 ||
    a.turnaround !== 'standard' ||
    !!a.drawing
  );
}
type EngineQuote = { id: string; expires_at: number };
type QuoteJob = {
  id: string;
  status: 'queued' | 'running' | 'succeeded' | 'failed';
  result?: EngineQuote;
  message?: string;
};

function waitForQuotePoll(signal: AbortSignal): Promise<void> {
  signal.throwIfAborted();
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      signal.removeEventListener('abort', abort);
      resolve();
    }, 1000);
    function abort() {
      clearTimeout(timer);
      reject(signal.reason);
    }
    signal.addEventListener('abort', abort, { once: true });
  });
}

async function estimateModel(body: unknown, signal?: AbortSignal): Promise<EngineQuote> {
  const timeout = AbortSignal.timeout(10 * 60 * 1000);
  const bounded = signal ? AbortSignal.any([signal, timeout]) : timeout;
  let job = await request<QuoteJob>('/api/quotes/estimate', body, bounded);
  const path = `/api/quotes/jobs/${encodeURIComponent(job.id)}`;
  for (;;) {
    if (job.status === 'succeeded') {
      if (!job.result) throw new Error('Brakuje wyniku wyceny. Spróbuj ponownie.');
      return job.result;
    }
    if (job.status === 'failed')
      throw new Error(job.message || 'Nie udało się przygotować wyceny. Spróbuj ponownie.');
    if (job.status !== 'queued' && job.status !== 'running')
      throw new Error('Nieprawidłowy stan wyceny. Spróbuj ponownie.');
    await waitForQuotePoll(bounded);
    job = await request<QuoteJob>(path, undefined, bounded);
  }
}

function manualQuote(
  reason = 'Zespół sprawdzi możliwość wydruku, zakres wykonania i dostawę, a następnie przedstawi ofertę.',
): { checkout: CheckoutQuote; manual: boolean } {
  return {
    manual: true,
    checkout: {
      id: crypto.randomUUID(),
      expires_at: Math.floor(Date.now() / 1000) + 600,
      packing: { status: 'manual_review', parcels: [], packaging_grosze: 0 },
      offers: [],
      print_grosze: 0,
      print_lines: [],
      setup_grosze: 0,
      packaging_grosze: 0,
      production_demo: true,
      stripe_configured: false,
      furgonetka_configured: false,
      reason,
    },
  };
}

const engineQuotes = new Map<string, EngineQuote>();
export async function quoteModels(
  models: PrintModel[],
  delivery: DeliveryMethod,
  contact?: Partial<Contact>,
  manual = false,
  signal?: AbortSignal,
): Promise<{ checkout: CheckoutQuote; manual: boolean }> {
  if (!models.length || models.length > 10) throw new Error('Dodaj od 1 do 10 części.');
  const materials = await getMaterials();
  manual ||= models.some((model) => {
    const material = materials.find((item) => item.id === model.options.materialId);
    return (
      !model.analysis ||
      !!model.colorPreference ||
      model.analysis.dimensions_mm.some((axis) => axis > 500) ||
      !material?.compatible ||
      !material.colors.some(
        (color) => color.code === model.options.color || color.name === model.options.color,
      ) ||
      needsManualReview(model)
    );
  });
  if (manual) return manualQuote();
  const parts = [];
  for (const model of models) {
    let quoteId: string | undefined;
    if (!manual) {
      const material = materials.find((item) => item.id === model.options.materialId)!;
      const color = material.colors.find(
        (item) => item.code === model.options.color || item.name === model.options.color,
      )!;
      const body = {
        model_id: model.analysis!.id,
        material_id: material.id,
        color_code: color.code,
        options: {
          quantity: model.options.quantity,
          quality: model.options.quality,
          wall_loops: model.advanced.wallCount,
          infill_percent: model.advanced.infill,
        },
      };
      const key = JSON.stringify(body);
      let estimate = engineQuotes.get(key);
      if (!estimate || estimate.expires_at <= Date.now() / 1000 + 30) {
        try {
          estimate = await estimateModel(body, signal);
        } catch (error) {
          if (
            error instanceof ApiError &&
            error.status === 422 &&
            error.code === 'manual_quote_required'
          )
            return manualQuote(error.message);
          throw error;
        }
        if (engineQuotes.size >= 64) engineQuotes.delete(engineQuotes.keys().next().value!);
        engineQuotes.set(key, estimate);
      }
      quoteId = estimate.id;
    }
    parts.push({
      id: model.id,
      quantity: model.options.quantity,
      analysis_id: model.analysis?.id,
      quote_id: quoteId,
    });
  }
  let checkout: CheckoutQuote;
  try {
    checkout = await request<CheckoutQuote>(
      '/api/checkout/quote',
      { parts, delivery, contact, manual },
      signal,
    );
  } catch (error) {
    if ((error as { status?: number }).status === 409) engineQuotes.clear();
    throw error;
  }
  return { checkout, manual: manual || checkout.manual === true };
}
async function calculateQuote(
  contact: Contact | undefined,
  printOnly: boolean,
  signal?: AbortSignal,
): Promise<Quote> {
  const cart = await mock.getCart();
  const { checkout, manual } = await quoteModels(
    cart.models,
    printOnly ? 'pickup' : cart.delivery,
    contact ?? { postalCode: cart.postalCode },
    false,
    signal,
  );
  const offer = checkout.offers[0];
  const subtotal = checkout.print_grosze / 100;
  const delivery = offer ? offer.price_grosze / 100 : 0;
  const count = cart.models.reduce((sum, model) => sum + model.options.quantity, 0);
  const quote: Quote = {
    id: checkout.id,
    version: 1,
    fingerprint: '',
    leadTime: 'Termin potwierdzi zespół',
    unitPrice: count ? subtotal / count : 0,
    lines: checkout.print_lines.map((line) => ({
      modelId: line.model_id,
      name: cart.models.find((model) => model.id === line.model_id)?.name ?? line.model_id,
      quantity: line.quantity,
      unitPrice: line.unit_grosze / 100,
      total: line.total_grosze / 100,
    })),
    checkout,
    subtotal,
    delivery,
    total: printOnly ? subtotal : subtotal + checkout.packaging_grosze / 100 + delivery,
    packages: checkout.packing.parcels.length,
    deliveryPending: printOnly || !offer,
    manual: manual || (!printOnly && checkout.packing.status === 'manual_review'),
    reason:
      checkout.reason ??
      checkout.packing.reason ??
      (manual ? 'Wymagania, materiał lub dostępność wymagają indywidualnej wyceny.' : undefined),
  };
  return mock.saveQuote(quote, cart);
}
export function startCheckout(quoteId: string, offerId: string) {
  return request<{ url: string; session_id: string; order_id: string }>('/api/checkout/session', {
    quote_id: quoteId,
    offer_id: offerId,
    legal_version: LEGAL_VERSION,
  });
}
export async function verifyCheckout(sessionId: string) {
  const status = await request<StripeStatus>(
    (sessionId.startsWith('pi_') ? '/api/checkout/intent/' : '/api/checkout/session/') +
      encodeURIComponent(sessionId),
  );
  mock.applyCheckoutStatus(sessionId, status);
  return status;
}
export async function startOrderCheckout(quote: Quote, contact: Contact, offerId: string) {
  const checkout = quote.checkout;
  const offer = checkout?.offers.find((offer) => offer.id === offerId);
  if (!checkout || !offer || quote.manual) throw new Error('Sprawdź wycenę i dostawę.');
  const result = await startCheckout(checkout.id, offerId);
  const url = new URL(result.url);
  if (url.protocol !== 'https:' || url.hostname !== 'checkout.stripe.com')
    throw new Error('Nieprawidłowy adres płatności.');
  const amountGrosze = checkout.print_grosze + checkout.packaging_grosze + offer.price_grosze;
  const snapshot = { ...quote, delivery: offer.price_grosze / 100, total: amountGrosze / 100 };
  const order = mock.saveSandboxOrder(snapshot, contact, {
    sessionId: result.session_id,
    url: result.url,
    quoteId: checkout.id,
    amountGrosze,
    offer,
  });
  return { ...result, order: { ...order, id: result.order_id } };
}
export async function refreshOrderPayment(orderId: string): Promise<Order> {
  await request(`/api/account/orders/${encodeURIComponent(orderId)}/payment-status`, {});
  return getOrder(orderId);
}
export async function openOrderPayment(orderId: string): Promise<{ url: string }> {
  const order = await getOrder(orderId);
  if (['paid', 'refunded'].includes(order.payment))
    throw new Error('Zamówienie jest już opłacone.');
  const reference = order.paymentReference;
  if (!reference || !/^(pi_|cs_)[a-zA-Z0-9_]+$/.test(reference))
    throw new Error('Brak aktywnej płatności. Skontaktuj się z obsługą.');
  return {
    url:
      '/zamowienie?' +
      (reference.startsWith('pi_') ? 'payment_intent=' : 'stripe_session=') +
      encodeURIComponent(reference),
  };
}

export function getPickupPoints(search: string) {
  return request<{ points: PickupPoint[] }>('/api/shipping/points', { search });
}

export function getIntegrationStatus() {
  return request<{ stripe_configured: boolean; furgonetka_configured: boolean }>(
    '/api/checkout/status',
  );
}

export function getStripeConfig() {
  return request<{ publishable_key: string | null }>('/api/checkout/config');
}
export async function startOrderIntent(quote: Quote, contact: Contact, offerId: string) {
  const checkout = quote.checkout;
  const offer = checkout?.offers.find((o) => o.id === offerId);
  if (!checkout || !offer || quote.manual) throw new Error('Sprawdź wycenę i dostawę.');
  const result = await request<{
    client_secret: string;
    payment_intent_id: string;
    order_id: string;
    total_grosze: number;
  }>('/api/checkout/intent', {
    quote_id: checkout.id,
    offer_id: offerId,
    legal_version: LEGAL_VERSION,
  });
  const expected = checkout.print_grosze + checkout.packaging_grosze + offer.price_grosze;
  if (result.total_grosze !== expected) throw new Error('Kwota płatności różni się od wyceny.');
  const snapshot = { ...quote, delivery: offer.price_grosze / 100, total: expected / 100 };
  const order = mock.saveSandboxOrder(snapshot, contact, {
    sessionId: result.payment_intent_id,
    url: '',
    quoteId: checkout.id,
    amountGrosze: expected,
    offer,
  });
  return { ...result, order: { ...order, id: result.order_id } };
}

export async function analyzeModel(file: File) {
  const response = await apiFetch('/api/models', {
    method: 'POST',
    headers: { 'X-Model-Filename': encodeURIComponent(file.name) },
    body: file,
  });
  const data = await response.json();
  if (!response.ok) throw new Error(data.message ?? 'Nie udało się przeanalizować modelu.');
  return data as NonNullable<import('./types').PrintModel['analysis']>;
}
