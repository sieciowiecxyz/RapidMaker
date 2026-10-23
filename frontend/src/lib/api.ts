// Wspólny adapter: operator ma sesję serwerową; konta klientów nadal są prototypem.
export {
  getPrintOptions,
  getMaterials,
  getDemoModel,
  getQuote,
  getCheckoutOrder,
  getCart,
  addModel,
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
    throw new Error(data.message ?? data.error ?? 'Usługa wyceny jest niedostępna.');
  return data as T;
}
export function quoteCart(contact?: Contact, signal?: AbortSignal): Promise<Quote> {
  return calculateQuote(contact, false, signal);
}

export function quotePrintCart(): Promise<Quote> {
  return calculateQuote(undefined, true);
}

async function calculateQuote(
  contact: Contact | undefined,
  printOnly: boolean,
  signal?: AbortSignal,
): Promise<Quote> {
  const cart = await mock.getCart();
  const quote = await mock.quoteCart();
  const checkout = await request<CheckoutQuote>(
    '/api/checkout/quote',
    {
      parts: cart.models.map((model) => ({
        id: model.id,
        quantity: model.options.quantity,
        analysis_id: model.analysis?.id,
        dimensions_mm: model.shipping?.dimensions_mm ?? null,
        weight_g: model.shipping?.weight_g ?? null,
      })),
      delivery: printOnly ? 'pickup' : cart.delivery,
      manual: quote.manual,
      contact: contact ?? { postalCode: cart.postalCode },
    },
    signal,
  );
  const offer = checkout.offers[0];
  const subtotal = checkout.print_grosze / 100;
  const delivery = offer ? offer.price_grosze / 100 : 0;
  return {
    ...quote,
    lines: checkout.print_lines.map((line) => ({
      modelId: line.model_id,
      name: cart.models.find((m) => m.id === line.model_id)?.name ?? line.model_id,
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
    manual: quote.manual || (!printOnly && checkout.packing.status === 'manual_review'),
    reason: printOnly ? quote.reason : (checkout.reason ?? checkout.packing.reason ?? quote.reason),
  };
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
  const response = await fetch('/api/models/analyze', {
    method: 'POST',
    headers: { 'X-Model-Filename': encodeURIComponent(file.name) },
    body: file,
  });
  const data = await response.json();
  if (!response.ok) throw new Error(data.message ?? 'Nie udało się przeanalizować modelu.');
  return data as NonNullable<import('./types').PrintModel['analysis']>;
}
