import { ApiError, apiFetch, apiRequest } from './http';

export type Parcel = { outer_mm: [number, number, number]; weight_g: number };
export type PickupDate = { date: string; min_time: string; max_time: string };
export type ShipmentAction = {
  id: string;
  kind: string;
  state: string;
  request_id: string;
  result: { error?: string };
};
export type Shipment = {
  id: string;
  version: number;
  state: string;
  last_error: string | null;
  input: { parcels: Parcel[]; refund_reviewed: boolean };
  quote: {
    id: string;
    expires_at: number;
    price_grosze: number;
    collected_shipping_grosze: number;
    difference_grosze: number;
  } | null;
  packages: { id: string; reference: string; provider_id: string | null; state: string }[];
};
export type ShipmentSnapshot = {
  order: {
    status: string;
    paid_at: number | null;
    contact: {
      name?: string;
      street?: string;
      city?: string;
      postalCode?: string;
      postal_code?: string;
    };
    fulfillment: {
      delivery: string | { method: string };
      offer?: { name?: string };
      packing?: {
        parcels?: {
          outer_mm?: [number, number, number];
          dimensions_mm?: [number, number, number];
          weight_g: number;
        }[];
      };
    };
  };
  shipment: Shipment | null;
};
export type ShippingIntent = {
  operation: 'book' | 'cancel' | 'pickup';
  body: Record<string, unknown>;
};
export const shipmentPath = (id: string) => `/api/admin/orders/${encodeURIComponent(id)}/shipment`;
export function shippingRequest<T>(id: string, suffix = '', method = 'GET', body?: unknown) {
  return apiRequest<T>(shipmentPath(id) + suffix, method, body);
}
// Only command identifiers and parcel IDs are retained, never customer data or credentials.
export function shippingIntent(id: string, value?: ShippingIntent | null): ShippingIntent | null {
  const key = `rapidmaker:shipment-intent:${id}`;
  if (value === null) sessionStorage.removeItem(key);
  else if (value) sessionStorage.setItem(key, JSON.stringify(value));
  const saved = sessionStorage.getItem(key);
  if (!saved) return null;
  const parsed = JSON.parse(saved) as ShippingIntent;
  if (
    !parsed ||
    typeof parsed !== 'object' ||
    !['book', 'cancel', 'pickup'].includes(parsed.operation) ||
    !parsed.body?.request_id
  )
    throw new Error('Nieprawidłowy zapis operacji nadania.');
  return parsed;
}
export async function downloadLabel(orderId: string, packageId: string) {
  const response = await apiFetch(
    `${shipmentPath(orderId)}/labels/${encodeURIComponent(packageId)}?format=a6`,
  );
  if (!response.ok) {
    const problem = await response.json().catch(() => null);
    throw new ApiError(response.status, problem?.message ?? 'Nie udało się pobrać etykiety.');
  }
  if (!response.headers.get('content-type')?.includes('application/pdf'))
    throw new Error('Serwer nie zwrócił etykiety PDF.');
  const url = URL.createObjectURL(await response.blob());
  const link = document.createElement('a');
  link.href = url;
  link.download = `rapidmaker-${packageId}.pdf`;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
