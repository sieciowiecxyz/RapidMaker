import { LEGAL_VERSION } from './legal';
import { apiRequest, apiFetch, ApiError } from './http';
import type { Contact, DeliveryMethod, PrintModel, Material } from './types';
export type ManualOffer = {
  version: number;
  state: 'awaiting_quote' | 'offered' | 'payment_started' | 'expired' | 'paid';
  valid_until: number | null;
  amount_grosze: number | null;
  shipping_grosze: number | null;
  scope: string | null;
  notes?: string;
  payment_started?: boolean;
  payment_state?: string | null;
};
export type ManualPayment = {
  attempt_id: string;
  order_id: string;
  state:
    | 'creating'
    | 'unknown'
    | 'open'
    | 'processing'
    | 'paid'
    | 'failed'
    | 'expired'
    | 'manual_review';
  client_secret: string | null;
  payment_intent_id: string | null;
  total_grosze: number;
};
export function manualRequestBody(
  models: PrintModel[],
  contact: Contact,
  method: DeliveryMethod,
  notes: string,
  materials: Material[],
) {
  if (!models.length || models.length > 10 || models.some((model) => !model.analysis?.id))
    throw new Error('Dodaj poprawne pliki i poczekaj na ich analizę.');
  return {
    parts: models.map((model) => {
      const material = materials.find((item) => item.id === model.options.materialId);
      const color = material?.colors.find(
        (item) => item.code === model.options.color || item.name === model.options.color,
      );
      if (!material || (!color?.code && !model.colorPreference))
        throw new Error(`${model.name}: wybierz materiał i kolor z katalogu.`);
      return {
        model_id: model.analysis!.id,
        quantity: model.options.quantity,
        material_id: model.options.materialId,
        color_code: model.colorPreference ? '' : color!.code!,
        ...(model.colorPreference ? { requested_color: model.colorPreference } : {}),
        options: {
          quality: model.options.quality,
          wall_loops: model.advanced.wallCount,
          infill_percent: model.advanced.infill,
        },
      };
    }),
    contact: {
      name: contact.name.trim(),
      email: contact.email,
      phone: contact.phone.trim(),
      country: 'PL',
      street: contact.street.trim(),
      apartment: '',
      postal_code: contact.postalCode.trim(),
      city: contact.city.trim(),
      invoice: contact.company
        ? {
            company: contact.company.trim(),
            nip: contact.nip?.trim(),
            street: contact.street.trim(),
            postal_code: contact.postalCode.trim(),
            city: contact.city.trim(),
            country: 'PL',
          }
        : null,
    },
    delivery: { method, ...(method === 'locker' ? { point_id: contact.pickupPoint } : {}) },
    notes,
  };
}
export async function createManualRequest(
  input: ReturnType<typeof manualRequestBody> & { request_id: string },
) {
  const response = await apiFetch('/api/account/manual-quotes', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  });
  const data = await response.json();
  if (!response.ok)
    throw Object.assign(
      new ApiError(response.status, data.message ?? 'Nie udało się zapisać zgłoszenia.', data.code),
      { fieldErrors: data.field_errors },
    );
  if (typeof data.order_id !== 'string' || !data.manual_offer)
    throw new Error('Nie potwierdzono zapisu zgłoszenia. Ponów to samo zgłoszenie.');
  return data as { order_id: string; manual_offer: ManualOffer };
}
export function publishManualOffer(
  id: string,
  input: { version: number; amount_grosze: number; shipping_grosze: number; scope: string },
) {
  return apiRequest(`/api/admin/orders/${encodeURIComponent(id)}/quote`, 'POST', input);
}
export function manualPayment(id: string, version: number, requestId: string) {
  return apiRequest<ManualPayment>(
    `/api/account/orders/${encodeURIComponent(id)}/payment`,
    'POST',
    { version, request_id: requestId, legal_version: LEGAL_VERSION },
  );
}
export function validOfferAmount(offer: ManualOffer): number | null {
  const amount = offer.amount_grosze;
  if (amount === null) return null;
  if (
    !Number.isSafeInteger(amount) ||
    amount < 1 ||
    !Number.isSafeInteger(offer.shipping_grosze) ||
    offer.shipping_grosze! < 0 ||
    offer.shipping_grosze! > amount
  )
    throw new Error('Nieprawidłowa kwota oferty. Odśwież zamówienie.');
  return amount;
}
