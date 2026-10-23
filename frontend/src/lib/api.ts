// Wspólny adapter: operator ma sesję serwerową; konta klientów nadal są prototypem.
export {
  getPrintOptions,
  getDemoModel,
  getCheckoutOrder,
  getCart,
  updateModel,
  removeModel,
  setDelivery,
  getDeliveryOptions,
  checkout,
  retryPayment,
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
  updateMaterial,
  getSystemSettings,
  updateSystemSettings,
  retryJob,
  getDemoScenario,
  setDemoScenario,
  resetDemo,
} from './mock';

import * as mock from './mock';
import { operatorSession, logoutOperator } from './session';
export { loginOperator } from './session';
import type {
  Session,
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
export async function getSession(): Promise<Session | null> {
  const operator = await operatorSession();
  if (operator) return operator;
  const client = await mock.getSession();
  return client?.role === 'client' ? client : null;
}
export async function getDemoAccounts() {
  return (await mock.getDemoAccounts()).filter((account) => account.role === 'client');
}
export async function requestLogin(email: string, register = false) {
  const account = (await mock.getDemoAccounts()).find(
    (item) => item.email === email.trim().toLowerCase(),
  );
  if (account?.role === 'admin')
    throw new Error('Administrator loguje się kluczem dostępu poniżej.');
  return mock.requestLogin(email, register);
}
export async function confirmLogin(challengeId: string, expired = false) {
  const session = await mock.confirmLogin(challengeId, expired);
  if (session.role === 'admin') {
    await mock.logout();
    throw new Error('Administrator wymaga logowania przez serwer.');
  }
  return session;
}
export async function logout() {
  await logoutOperator();
  await mock.logout();
}

async function request<T>(path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const response = await fetch(path, {
    signal,
    method: body === undefined ? 'GET' : 'POST',
    headers: { 'Content-Type': 'application/json' },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  const data = await response.json();
  if (!response.ok)
    throw Object.assign(
      new Error(data.message ?? data.error ?? 'Usługa wyceny jest niedostępna.'),
      { status: response.status },
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
      inStock: color.in_stock,
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
      !material?.compatible ||
      !material.colors.some(
        (color) =>
          (color.code === model.options.color || color.name === model.options.color) &&
          color.inStock,
      ) ||
      needsManualReview(model)
    );
  });
  if (manual)
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
        reason:
          'Wymagania, materiał lub dostępność wymagają indywidualnej wyceny. Prześlij plik i wybierz dostępny materiał, aby otrzymać cenę automatycznie.',
      },
    };
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
        estimate = await request<EngineQuote>('/api/quotes/estimate', body, signal);
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
  return request<{ url: string; session_id: string }>('/api/checkout/session', {
    quote_id: quoteId,
    offer_id: offerId,
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
  return { ...result, order };
}
export async function refreshOrderPayment(orderId: string): Promise<Order> {
  const order = await mock.getOrder(orderId);
  if (!order.sandboxPayment) throw new Error('To zamówienie nie ma płatności Stripe.');
  await verifyCheckout(order.sandboxPayment.sessionId);
  return mock.getOrder(orderId);
}
export async function openOrderPayment(orderId: string): Promise<{ url: string }> {
  const order = await mock.getOrder(orderId);
  if (!order.sandboxPayment || order.status === 'cancelled')
    throw new Error('To zamówienie nie wymaga płatności.');
  const status = await verifyCheckout(order.sandboxPayment.sessionId);
  if (status.status === 'paid')
    throw new Error('Zamówienie jest już opłacone. Odśwież jego status.');
  if (status.checkout_status !== 'open') throw new Error('Sesja wygasła. Utwórz nową wycenę.');
  if (order.sandboxPayment.sessionId.startsWith('pi_')) {
    return {
      url: '/zamowienie?payment_intent=' + encodeURIComponent(order.sandboxPayment.sessionId),
    };
  }
  const url = new URL(status.url);
  if (url.protocol !== 'https:' || url.hostname !== 'checkout.stripe.com')
    throw new Error('Nieprawidłowy adres płatności.');
  return { url: url.href };
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
    total_grosze: number;
  }>('/api/checkout/intent', { quote_id: checkout.id, offer_id: offerId });
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
  return { ...result, order };
}

export async function analyzeModel(file: File) {
  const response = await fetch('/api/models', {
    method: 'POST',
    headers: { 'X-Model-Filename': encodeURIComponent(file.name) },
    body: file,
  });
  const data = await response.json();
  if (!response.ok) throw new Error(data.message ?? 'Nie udało się przeanalizować modelu.');
  return data as NonNullable<import('./types').PrintModel['analysis']>;
}
