export const stages = [
  'Nowe',
  'Przygotowanie',
  'Drukowanie',
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
  price: number;
  amount: number;
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
  material_id: string;
  color_code: string;
  options?: { quality?: string; wall_loops?: number; infill_percent?: number };
};
export type Order = {
  id: string;
  client: string;
  email: string;
  date: string;
  due: string;
  status: Stage;
  state: string;
  paid: boolean;
  payment: string;
  amount: number;
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
  payment_state: string | null;
  amount_grosze: number;
  created_at: number;
  paid_at: number | null;
  contact: {
    name?: string;
    email?: string;
    street?: string;
    postalCode?: string;
    city?: string;
    delivery_address?: string;
  };
  lines: { model_id: string; quantity: number; unit_grosze: number; total_grosze: number }[];
  fulfillment?: { delivery?: string | { method?: string }; offer?: { name?: string } };
};
const labels: Record<string, Stage> = {
  awaiting_payment: 'Nowe',
  paid: 'Przygotowanie',
  in_production: 'Drukowanie',
  ready: 'Pakowanie',
  shipped: 'Wysłane',
  completed: 'Zakończone',
  cancelled: 'Anulowane',
};
export function orderFromServer(row: ServerOrder): Order {
  if (!Number.isSafeInteger(row.amount_grosze) || row.amount_grosze < 0)
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
    due: '',
    status: labels[row.status] || 'Nowe',
    state: row.status,
    paid,
    payment:
      row.payment_state === 'refunded'
        ? 'Zwrócona'
        : row.payment_state === 'partially_refunded'
          ? 'Częściowy zwrot'
          : paid
            ? 'Opłacone'
            : row.payment_state === 'failed'
              ? 'Nieudana'
              : 'Oczekuje na płatność',
    amount: row.amount_grosze / 100,
    attention:
      row.payment_state === 'manual_review'
        ? 'Sprawdź płatność'
        : row.status === 'ready'
          ? 'Gotowe do odbioru lub nadania'
          : '',
    delivery: method === 'pickup' ? 'Odbiór osobisty' : row.fulfillment?.offer?.name || 'Dostawa',
    pickup: method === 'pickup',
    address:
      row.contact.delivery_address ||
      [row.contact.street, [row.contact.postalCode, row.contact.city].filter(Boolean).join(' ')]
        .filter(Boolean)
        .join(', '),
    models: row.lines.map((l) => ({
      id: l.model_id,
      name: 'Model ' + l.model_id,
      material: '—',
      color: '—',
      qty: l.quantity,
      price: l.unit_grosze / 100,
      amount: l.total_grosze / 100,
      done: 0,
      quality: '—',
      strength: '—',
    })),
    notes: [],
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
  const state =
    o.state === 'paid'
      ? 'in_production'
      : o.state === 'in_production'
        ? 'ready'
        : o.state === 'ready' && o.pickup
          ? 'completed'
          : null;
  return state ? { state, label: labels[state] } : null;
}
export const total = (o: Order) => o.amount;
export const pieces = (o: Order) => o.models.reduce((s, m) => s + m.qty, 0);
export const money = (n: number) =>
  new Intl.NumberFormat('pl-PL', { style: 'currency', currency: 'PLN' }).format(n);
// Prefix dangerous cells inside quotes (OWASP CSV Injection).
export function csvCell(value: string): string {
  const text =
    /^[\s\uFEFF]*[=+\-@＝＋－＠]|[\t\r\n]/u.test(value) || /^[＝＋－＠]/u.test(value)
      ? '\t' + value
      : value;
  return '"' + text.replaceAll('"', '""') + '"';
}
