import type { Production, InvoiceDraft } from './production';
import type { ManualOffer } from '../lib/manualQuotes';
export const stages = [
  'Oczekuje na wycenę',
  'Oferta wygasła',
  'Nowe',
  'Przygotowanie',
  'Drukowanie',
  'Kontrola jakości',
  'Pakowanie',
  'Wysłane',
  'Zakończone',
  'Anulowane',
] as const;
export type Stage = (typeof stages)[number];
export type Model = {
  id: string;
  name: string;
  material: string;
  color: string;
  qty: number;
  quality: string;
  strength: string;
  price: number | null;
  amount: number | null;
  done: number;
};
export type Message = {
  id: string;
  sequence: number;
  operator: boolean;
  body: string;
  created_at: number;
};
export type OrderFile = {
  model_id: string;
  filename: string;
  available: boolean;
  source_url?: string;
  preview_url?: string;
  material_id: string;
  color_code: string;
  options?: { quality?: string; wall_loops?: number; infill_percent?: number };
};
export type Order = {
  id: string;
  production?: Production | null;
  invoiceDraft?: InvoiceDraft | null;
  invoiceContact: { buyer: string; address: string; nip: string };
  client: string;
  email: string;
  date: string;
  due: string;
  status: Stage;
  state: string;
  paid: boolean;
  payment: string;
  amount: number | null;
  amountGrosze: number | null;
  manualOffer?: ManualOffer;
  attention: string;
  delivery: string;
  pickup: boolean;
  address: string;
  models: Model[];
  notes: string[];
  history: string[];
  message: string;
  draft: string;
  messages: Message[];
  nextAfter: number | null;
  files: OrderFile[];
};
export type ServerOrder = {
  id: string;
  status: string;
  production?: Production | null;
  invoice_draft?: InvoiceDraft | null;
  payment_state: string | null;
  amount_grosze: number | null;
  manual_offer?: ManualOffer;
  created_at: number;
  paid_at: number | null;
  contact: {
    name?: string;
    email?: string;
    street?: string;
    postalCode?: string;
    postal_code?: string;
    city?: string;
    delivery_address?: string;
    invoice?: {
      company?: string;
      nip?: string;
      street?: string;
      postal_code?: string;
      city?: string;
    };
  };
  lines: {
    model_id: string;
    quantity: number;
    unit_grosze?: number | null;
    total_grosze?: number | null;
    filename?: string;
  }[];
  fulfillment?: { delivery?: string | { method?: string }; offer?: { name?: string } };
};
const labels: Record<string, Stage> = {
  awaiting_payment: 'Nowe',
  awaiting_quote: 'Oczekuje na wycenę',
  quote_expired: 'Oferta wygasła',
  paid: 'Przygotowanie',
  in_production: 'Drukowanie',
  ready: 'Pakowanie',
  shipped: 'Wysłane',
  completed: 'Zakończone',
  cancelled: 'Anulowane',
};
export function orderFromServer(row: ServerOrder): Order {
  if (
    !(row.amount_grosze === null && row.manual_offer) &&
    (!Number.isSafeInteger(row.amount_grosze) || row.amount_grosze! < 0)
  )
    throw new Error('Nieprawidłowa kwota zamówienia.');
  const method =
    typeof row.fulfillment?.delivery === 'string'
      ? row.fulfillment.delivery
      : row.fulfillment?.delivery?.method;
  const paid = row.paid_at !== null && row.paid_at > 0;
  return {
    id: row.id,
    client: row.contact.name || row.contact.email || 'Klient',
    email: row.contact.email || '',
    date: new Date(row.created_at * 1000).toISOString().slice(0, 10),
    due: row.production?.due_date ?? '',
    production: row.production,
    invoiceDraft: row.invoice_draft,
    invoiceContact: {
      buyer: row.contact.invoice?.company || row.contact.name || row.contact.email || 'Klient',
      address: row.contact.invoice
        ? [
            row.contact.invoice.street,
            [row.contact.invoice.postal_code, row.contact.invoice.city].filter(Boolean).join(' '),
          ]
            .filter(Boolean)
            .join(', ')
        : [
            row.contact.street,
            [row.contact.postalCode ?? row.contact.postal_code, row.contact.city]
              .filter(Boolean)
              .join(' '),
          ]
            .filter(Boolean)
            .join(', '),
      nip: row.contact.invoice?.nip ?? '',
    },
    status:
      ['paid', 'in_production'].includes(row.status) && row.production
        ? (
            {
              preparation: 'Przygotowanie',
              printing: 'Drukowanie',
              qc: 'Kontrola jakości',
              packing: 'Pakowanie',
            } as const
          )[row.production.stage]
        : labels[row.status] || 'Nowe',
    state: row.status,
    paid,
    payment:
      row.status === 'awaiting_quote'
        ? 'Do wyceny'
        : row.payment_state === 'refunded'
          ? 'Zwrócona'
          : row.payment_state === 'partially_refunded'
            ? 'Częściowy zwrot'
            : paid
              ? 'Opłacone'
              : row.payment_state === 'failed'
                ? 'Nieudana'
                : 'Oczekuje na płatność',
    amount: row.amount_grosze === null ? null : row.amount_grosze / 100,
    manualOffer: row.manual_offer,
    amountGrosze: row.amount_grosze,
    attention:
      row.status === 'awaiting_quote'
        ? 'Wycena indywidualna'
        : row.payment_state === 'manual_review'
          ? 'Sprawdź płatność'
          : row.status === 'ready'
            ? 'Gotowe do odbioru lub nadania'
            : '',
    delivery: method === 'pickup' ? 'Odbiór osobisty' : row.fulfillment?.offer?.name || 'Dostawa',
    pickup: method === 'pickup',
    address:
      row.contact.delivery_address ||
      [
        row.contact.street,
        [row.contact.postalCode ?? row.contact.postal_code, row.contact.city]
          .filter(Boolean)
          .join(' '),
      ]
        .filter(Boolean)
        .join(', '),
    models: row.lines.map((l) => ({
      id: l.model_id,
      name: l.filename || 'Model ' + l.model_id,
      material: '—',
      color: '—',
      qty: l.quantity,
      price: l.unit_grosze == null ? null : l.unit_grosze / 100,
      amount: l.total_grosze == null ? null : l.total_grosze / 100,
      done: row.production?.progress.find((p) => p.model_id === l.model_id)?.done ?? 0,
      quality: '—',
      strength: '—',
    })),
    notes: row.production?.private_notes ? [row.production.private_notes] : [],
    history: [
      'Złożono zamówienie: ' + new Date(row.created_at * 1000).toLocaleString('pl-PL'),
      ...(paid
        ? ['Płatność potwierdzona: ' + new Date(row.paid_at! * 1000).toLocaleString('pl-PL')]
        : []),
    ],
    message: 'Otwórz rozmowę zamówienia',
    draft: '',
    messages: [],
    nextAfter: null,
    files: [],
  };
}
export function nextStatus(o: Order): { state: string; label: Stage } | null {
  if (['paid', 'in_production'].includes(o.state)) {
    const stage = o.production?.stage ?? (o.state === 'in_production' ? 'printing' : 'preparation');
    const next = {
      preparation: ['production:printing', 'Drukowanie'],
      printing: ['production:qc', 'Kontrola jakości'],
      qc: ['production:packing', 'Pakowanie'],
      packing: ['ready', 'Pakowanie'],
    }[stage];
    return { state: next[0], label: next[1] as Stage };
  }
  return o.state === 'ready' && o.pickup ? { state: 'completed', label: 'Zakończone' } : null;
}
export const total = (o: Order) => o.amount;
export const pieces = (o: Order) => o.models.reduce((s, m) => s + m.qty, 0);
export const money = (n: number | null) =>
  n === null
    ? 'Do wyceny'
    : new Intl.NumberFormat('pl-PL', { style: 'currency', currency: 'PLN' }).format(n);
// Prefix dangerous cells inside quotes (OWASP CSV Injection).
export function csvCell(value: string): string {
  const text =
    /^[\s\uFEFF]*[=+\-@＝＋－＠]|[\t\r\n]/u.test(value) || /^[＝＋－＠]/u.test(value)
      ? '\t' + value
      : value;
  return '"' + text.replaceAll('"', '""') + '"';
}

// Display only; requests and CSV always retain the complete identifier.
export const shortId = (id: string) =>
  id.length > 16 ? `${id.slice(0, 8)}…${id.slice(-4)}`.toUpperCase() : id;
