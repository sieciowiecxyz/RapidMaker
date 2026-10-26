import { apiRequest } from '../lib/http';
import type { Order } from './data';

export type Production = {
  revision: number;
  stage: 'preparation' | 'printing' | 'qc' | 'packing';
  due_date: string | null;
  private_notes: string;
  progress: { model_id: string; done: number }[];
  qc_passed: boolean;
};
export type InvoiceDraft = {
  revision: number;
  buyer: string;
  address: string;
  nip: string;
  created: string;
  lines: { name: string; qty: number; price_grosze: number }[];
};
export const productionStages = [
  ['preparation', 'Przygotowanie'],
  ['printing', 'Drukowanie'],
  ['qc', 'Kontrola jakości'],
  ['packing', 'Pakowanie'],
] as const;
export function productionFor(order: Order): Production {
  return (
    order.production ?? {
      revision: 0,
      stage: order.state === 'in_production' ? 'printing' : 'preparation',
      due_date: null,
      private_notes: '',
      progress: [],
      qc_passed: false,
    }
  );
}
export const saveProduction = (id: string, input: Production) =>
  apiRequest<Production>(`/api/admin/orders/${encodeURIComponent(id)}/production`, 'PUT', input);
export const saveInvoice = (id: string, input: InvoiceDraft) =>
  apiRequest<InvoiceDraft>(
    `/api/admin/orders/${encodeURIComponent(id)}/invoice-draft`,
    'PUT',
    input,
  );
