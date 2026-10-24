import { apiRequest, ApiError } from './http';
export { hundredths } from './operator';

export type Refund = {
  id: string;
  order_id: string;
  request_id: string;
  amount_grosze: number;
  state: string;
  created_at: number;
};
type Request = { request_id: string; amount_grosze: number };
// Retain only the operation ID and amount so a reload cannot issue another partial refund.
const path = (id: string) => `/api/admin/orders/${encodeURIComponent(id)}/refunds`;
const intentKey = (id: string) => `rapidmaker:refund-intent:${id}`;
export function pendingRefund(id: string): Request | undefined {
  const saved = sessionStorage.getItem(intentKey(id));
  if (!saved) return undefined;
  const input = JSON.parse(saved) as Request;
  if (
    !input ||
    typeof input !== 'object' ||
    typeof input.request_id !== 'string' ||
    !/^[a-f0-9-]{36}$/.test(input.request_id) ||
    !Number.isSafeInteger(input.amount_grosze) ||
    input.amount_grosze < 1
  )
    throw new Error('Nieprawidłowy zapis operacji zwrotu.');
  return input;
}
function clear(id: string) {
  sessionStorage.removeItem(intentKey(id));
}
export const refundPending = (refund: Refund) =>
  !['succeeded', 'failed', 'canceled'].includes(refund.state);

export function refundTotals(refunds: Refund[], amount: number) {
  if (!Number.isSafeInteger(amount) || amount < 0)
    throw new Error('Nieprawidłowa kwota płatności.');
  let returned = 0;
  let reserved = 0;
  for (const refund of refunds) {
    if (!Number.isSafeInteger(refund.amount_grosze) || refund.amount_grosze <= 0)
      throw new Error('Nieprawidłowa kwota zwrotu z serwera.');
    if (refund.state === 'succeeded') returned += refund.amount_grosze;
    if (!['failed', 'canceled'].includes(refund.state)) reserved += refund.amount_grosze;
  }
  if (!Number.isSafeInteger(reserved) || reserved > amount)
    throw new Error('Kwoty zwrotów wymagają sprawdzenia. Odśwież zamówienie.');
  return { returned, pending: reserved - returned, remaining: amount - reserved };
}
export async function listRefunds(id: string): Promise<Refund[]> {
  const refunds = await apiRequest<Refund[]>(path(id));
  if (!Array.isArray(refunds)) throw new Error('Nieprawidłowa odpowiedź serwera.');
  const input = pendingRefund(id);
  if (
    input &&
    refunds.some(
      (r) => r.request_id === input.request_id && r.amount_grosze === input.amount_grosze,
    )
  )
    clear(id);
  return refunds;
}
export async function requestRefund(id: string, amount: number): Promise<Refund> {
  let input = pendingRefund(id);
  const retry = !!input;
  if (input && input.amount_grosze !== amount)
    throw new Error('Najpierw sprawdź wynik poprzedniego zwrotu.');
  if (!Number.isSafeInteger(amount) || amount < 1 || amount > 99_999_999)
    throw new Error('Podaj dodatnią kwotę zwrotu, nie większą niż 999 999,99 zł.');
  if (!input) {
    input = { request_id: crypto.randomUUID(), amount_grosze: amount };
    sessionStorage.setItem(intentKey(id), JSON.stringify(input));
  }
  try {
    const refund = await apiRequest<Refund>(path(id), 'POST', input);
    if (refund?.request_id !== input.request_id || refund.amount_grosze !== input.amount_grosze)
      throw new Error('Nie potwierdzono wyniku zlecenia. Sprawdź status przed kolejną operacją.');
    clear(id);
    return refund;
  } catch (error) {
    // A received validation/authorization rejection cannot have created this operation.
    if (!retry && error instanceof ApiError && [400, 401, 403, 404, 409].includes(error.status))
      clear(id);
    throw error;
  }
}
