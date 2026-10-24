import { apiRequest, ApiError } from './http';
import { getSession } from './session';
import type { Cart, Contact, Order, OrderStatus, Profile, PrintModel } from './types';

type RawOrder = {
  id: string;
  user_id?: string | null;
  status: string;
  payment_state?: string | null;
  amount_grosze: number;
  provider_id?: string | null;
  created_at: number;
  paid_at: number | null;
  contact: Partial<Contact> & { invoice?: { company?: string; nip?: string } };
  lines: { model_id: string; quantity: number; unit_grosze: number; total_grosze: number }[];
  fulfillment?: {
    delivery?: string | { method?: string };
    offer?: { name?: string; price_grosze?: number };
    packing?: { parcels: unknown[] };
    packaging_grosze?: number;
  };
};
type FileRecord = {
  model_id: string;
  filename: string;
  available: boolean;
  source_url?: string;
  preview_url?: string;
  material_id: string;
  color_code: string;
  quantity: number;
  dimensions_mm: [number, number, number];
  options?: {
    quality?: 'draft' | 'standard' | 'fine';
    wall_loops?: number;
    infill_percent?: number;
  };
};
type RawMessage = {
  id: string;
  sequence: number;
  operator: boolean;
  body: string;
  created_at: number;
};
type MessagePage = {
  messages: RawMessage[];
  next_after: number | null;
  read_sequence: number;
  unread: number;
};
type Refund = { request_id: string; amount_grosze: number; state: string };
export type LiveOrder = Order & {
  paymentReference?: string;
  files?: FileRecord[];
  deliveryKnown?: boolean;
  serverStatus?: string;
  refundPending?: boolean;
  refundNotice?: string;
  refundableAmount?: number;
};
const refundRequests = new Map<string, { request_id: string; amount_grosze: number }>();
window.addEventListener('rapidmaker:session-expired', () => refundRequests.clear());
const instant = (seconds: number) => new Date(seconds * 1000).toISOString();
const blankContact = (email = ''): Contact => ({
  name: '',
  email,
  phone: '',
  street: '',
  postalCode: '',
  city: '',
  company: '',
  nip: '',
  pickupPoint: '',
});
const statusMap: Record<string, OrderStatus> = {
  awaiting_payment: 'awaiting_payment',
  paid: 'preparing',
  in_production: 'printing',
  ready: 'ready',
  shipped: 'shipped',
  completed: 'completed',
};
const statusToServer: Partial<Record<OrderStatus, string>> = {
  printing: 'in_production',
  ready: 'ready',
  completed: 'completed',
};
async function scope() {
  const session = await getSession();
  if (session?.role === 'admin' && !session.adminVerified)
    throw new ApiError(403, 'Potwierdź dostęp kluczem passkey.');
  return {
    admin: session?.role === 'admin',
    prefix: session?.role === 'admin' ? '/api/admin/orders' : '/api/account/orders',
  };
}
function checkedAmount(value: number) {
  if (!Number.isSafeInteger(value) || value < 0) throw new Error('Nieprawidłowa kwota zamówienia.');
  return value;
}
function model(line: RawOrder['lines'][number], files: FileRecord[]): PrintModel {
  const matches = files.filter((f) => f.model_id === line.model_id),
    file = matches.length === 1 ? matches[0] : undefined;
  const name = matches[0]?.filename || 'Model ' + line.model_id;
  return {
    id: line.model_id,
    name,
    dimensions: file?.dimensions_mm?.join(' × ') || 'Brak danych',
    weight: 'Brak danych',
    format: /\.3mf$/i.test(name) ? '3MF' : /\.(step|stp)$/i.test(name) ? 'STEP' : 'STL',
    parts: 1,
    units: 'mm',
    options: {
      materialId: file?.material_id || '',
      color: file?.color_code || '',
      quantity: line.quantity,
      quality: file?.options?.quality || 'standard',
    },
    strength: 'custom',
    advanced: {
      wallCount: file?.options?.wall_loops ?? 0,
      infill: file?.options?.infill_percent ?? 0,
      pattern: 'grid',
      orientation: 'auto',
      supports: 'auto',
      finish: 'standard',
      fit: 'normal',
      insertCount: 0,
      insertSize: 'M3',
      turnaround: 'standard',
      drawing: null,
    },
    notes: matches.length > 1 ? 'Kilka konfiguracji tego modelu — sprawdź pliki zamówienia.' : '',
  };
}
function convert(
  raw: RawOrder,
  files: FileRecord[] = [],
  page?: MessagePage,
  refunds: Refund[] = [],
): LiveOrder {
  const amount = checkedAmount(raw.amount_grosze),
    fulfilled = raw.fulfillment;
  const method =
    typeof fulfilled?.delivery === 'string' ? fulfilled.delivery : fulfilled?.delivery?.method;
  const delivery = method === 'pickup' || method === 'locker' ? method : 'courier';
  const shipping = checkedAmount(fulfilled?.offer?.price_grosze ?? 0);
  const succeeded = refunds
    .filter((r) => r.state === 'succeeded')
    .reduce((sum, r) => sum + checkedAmount(r.amount_grosze), 0);
  const reserved = refunds
    .filter((r) => !['failed', 'canceled'].includes(r.state))
    .reduce((sum, r) => sum + checkedAmount(r.amount_grosze), 0);
  const pending = refunds.some((r) => !['succeeded', 'failed', 'canceled'].includes(r.state));
  const confirmed = raw.paid_at !== null && raw.paid_at > 0;
  const status = statusMap[raw.status];
  if (!status) throw new Error('Nieobsługiwany status zamówienia.');
  return {
    id: raw.id,
    paymentReference: raw.provider_id ?? undefined,
    number: raw.id,
    createdAt: instant(raw.created_at),
    ownerId: raw.user_id ?? null,
    contact: {
      ...blankContact(),
      ...raw.contact,
      company: raw.contact.invoice?.company || raw.contact.company || '',
      nip: raw.contact.invoice?.nip || raw.contact.nip || '',
    },
    models: raw.lines.map((line) => model(line, files)),
    quote: {
      id: raw.id,
      version: 0,
      fingerprint: '',
      total: amount / 100,
      unitPrice: 0,
      leadTime: 'Do potwierdzenia',
      subtotal: (amount - shipping) / 100,
      delivery: shipping / 100,
      packages: fulfilled?.packing?.parcels.length ?? 0,
      lines: raw.lines.map((line) => ({
        modelId: line.model_id,
        name: files.find((f) => f.model_id === line.model_id)?.filename || 'Model ' + line.model_id,
        quantity: line.quantity,
        unitPrice: checkedAmount(line.unit_grosze) / 100,
        total: checkedAmount(line.total_grosze) / 100,
      })),
      manual: false,
    },
    delivery,
    status,
    payment:
      succeeded >= amount && amount > 0
        ? 'refunded'
        : confirmed
          ? 'paid'
          : raw.payment_state === 'processing'
            ? 'pending'
            : raw.payment_state === 'failed'
              ? 'error'
              : 'unpaid',
    leadTime: 'Do potwierdzenia',
    tracking: '',
    label: '',
    document: '',
    notes: '',
    needsAttention: raw.payment_state === 'manual_review',
    messages:
      page?.messages.map((m) => ({
        id: m.id,
        sender: m.operator ? 'admin' : 'client',
        text: m.body,
        createdAt: instant(m.created_at),
        read: m.sequence <= page.read_sequence,
      })) ?? [],
    history: [
      { at: instant(raw.created_at), text: 'Złożono zamówienie' },
      ...(confirmed ? [{ at: instant(raw.paid_at!), text: 'Potwierdzono płatność' }] : []),
    ],
    offerReason: '',
    files,
    deliveryKnown: ['pickup', 'locker', 'courier'].includes(method ?? ''),
    serverStatus: raw.status,
    refundPending: pending,
    refundableAmount: Math.max(0, amount - reserved) / 100,
    refundNotice: pending
      ? 'Zwrot został zlecony i czeka na potwierdzenie Stripe.'
      : succeeded > 0
        ? `Potwierdzono zwrot ${(succeeded / 100).toFixed(2)} zł.`
        : undefined,
  };
}
async function rawOrder(id: string, access: Awaited<ReturnType<typeof scope>>): Promise<RawOrder> {
  if (access.admin) {
    const data = await apiRequest<{ orders: RawOrder[] }>(access.prefix);
    const row = data.orders.find((o) => o.id === id);
    if (!row) throw new ApiError(404, 'Nie znaleziono zamówienia w panelu.');
    return row;
  }
  return apiRequest<RawOrder>(`${access.prefix}/${encodeURIComponent(id)}`);
}
async function conversation(path: string): Promise<MessagePage> {
  const all: RawMessage[] = [];
  let after = 0;
  for (let i = 0; i < 10; i++) {
    const page = await apiRequest<MessagePage>(`${path}/messages?after=${after}&limit=100`);
    all.push(...page.messages);
    if (page.next_after === null) return { ...page, messages: all };
    if (page.next_after <= after) throw new Error('Nieprawidłowa kolejność wiadomości.');
    after = page.next_after;
  }
  throw new Error('Rozmowa przekracza limit tego widoku (1000 wiadomości).');
}
export async function getOrder(id: string): Promise<LiveOrder> {
  const access = await scope(),
    path = `${access.prefix}/${encodeURIComponent(id)}`;
  const [raw, files, page, refunds] = await Promise.all([
    rawOrder(id, access),
    apiRequest<{ files: FileRecord[] }>(path + '/files'),
    conversation(path),
    access.admin ? apiRequest<Refund[]>(path + '/refunds') : Promise.resolve([]),
  ]);
  const result = convert(raw, files.files, page, refunds);
  type Shipment = { packages: { tracking: { numbers?: string[]; number?: string } }[] };
  const delivery = await apiRequest<Shipment | { shipment: Shipment | null }>(path + '/shipment');
  const shipment = 'shipment' in delivery ? delivery.shipment : delivery;
  result.tracking =
    shipment?.packages
      .flatMap((p) => p.tracking.numbers ?? (p.tracking.number ? [p.tracking.number] : []))
      .join(', ') ?? '';

  return result;
}
export async function getOrders(): Promise<LiveOrder[]> {
  const access = await scope();
  const data = await apiRequest<{ orders: RawOrder[] }>(access.prefix);
  const result: LiveOrder[] = new Array(data.orders.length);
  let cursor = 0;
  await Promise.all(
    Array.from({ length: Math.min(4, data.orders.length) }, async () => {
      while (cursor < data.orders.length) {
        const index = cursor++,
          row = data.orders[index],
          path = `${access.prefix}/${encodeURIComponent(row.id)}`;
        const [raw, files] = await Promise.all([
          access.admin ? Promise.resolve(row) : rawOrder(row.id, access),
          apiRequest<{ files: FileRecord[] }>(path + '/files'),
        ]);
        result[index] = convert(raw, files.files);
      }
    }),
  );
  return result;
}
export async function sendMessage(id: string, text: string): Promise<LiveOrder> {
  const access = await scope(),
    body = text.trim();
  if (!body || new TextEncoder().encode(body).length > 4000)
    throw new Error('Wiadomość musi mieć od 1 do 4000 bajtów.');
  await apiRequest(`${access.prefix}/${encodeURIComponent(id)}/messages`, 'POST', { body });
  return getOrder(id);
}
export async function markMessagesRead(id: string): Promise<LiveOrder> {
  const access = await scope(),
    path = `${access.prefix}/${encodeURIComponent(id)}`,
    page = await conversation(path),
    last = page.messages.at(-1);
  if (last) await apiRequest(path + '/messages/read', 'POST', { sequence: last.sequence });
  return getOrder(id);
}
export async function updateOrder(
  id: string,
  patch: Partial<Pick<Order, 'status' | 'leadTime' | 'notes' | 'needsAttention'>>,
): Promise<LiveOrder> {
  const access = await scope();
  if (!access.admin) throw new ApiError(403, 'Ta operacja wymaga administratora.');
  if (Object.keys(patch).some((key) => key !== 'status'))
    throw new Error('Serwer obsługuje zmianę statusu; termin i notatki są niedostępne.');
  const status = patch.status && statusToServer[patch.status];
  if (!status) throw new Error('Ten status zmienia automatycznie płatność lub nadanie przesyłki.');
  await apiRequest(`${access.prefix}/${encodeURIComponent(id)}/status`, 'POST', { status });
  return getOrder(id);
}
export async function refundOrder(id: string): Promise<LiveOrder> {
  const access = await scope();
  if (!access.admin) throw new ApiError(403, 'Zwrot może zlecić tylko administrator.');
  const path = `${access.prefix}/${encodeURIComponent(id)}`;
  let input = refundRequests.get(id);
  if (!input) {
    const [raw, refunds] = await Promise.all([
      rawOrder(id, access),
      apiRequest<Refund[]>(path + '/refunds'),
    ]);
    const reserved = refunds
      .filter((r) => !['failed', 'canceled'].includes(r.state))
      .reduce((sum, r) => sum + checkedAmount(r.amount_grosze), 0);
    const amount = checkedAmount(raw.amount_grosze) - reserved;
    if (amount <= 0)
      throw new Error('Cała kwota jest już zwrócona lub zarezerwowana na oczekujące zwroty.');
    input = { request_id: crypto.randomUUID(), amount_grosze: amount };
    refundRequests.set(id, input);
  }
  await apiRequest<Refund>(path + '/refunds', 'POST', input);
  return getOrder(id);
}
export async function getProfile(): Promise<Profile> {
  const me = await apiRequest<{ user: { email: string } }>('/api/auth/me');
  const prefs = await apiRequest<{ email_order_updates: boolean; email_chat_messages: boolean }>(
    '/api/account/notifications',
  );
  return {
    contact: blankContact(me.user.email),
    notifications: {
      message: prefs.email_chat_messages,
      status: prefs.email_order_updates,
      shipped: prefs.email_order_updates,
      pickup: prefs.email_order_updates,
    },
  };
}
export async function updateProfile(value: Profile): Promise<Profile> {
  const current = await getProfile();
  if (
    Object.entries(current.contact).some(
      ([key, valueNow]) => value.contact[key as keyof Contact] !== valueNow,
    )
  )
    throw new Error('Zapisywanie danych kontaktowych konta nie jest jeszcze dostępne.');
  await apiRequest('/api/account/notifications', 'PUT', {
    email_order_updates: value.notifications.status,
    email_chat_messages: value.notifications.message,
  });
  return getProfile();
}
function unavailable(): never {
  throw new Error('Ta funkcja nie jest jeszcze połączona z serwerem.');
}
export async function reorder(_id: string): Promise<Cart> {
  return unavailable();
}
export async function deleteAccount(): Promise<void> {
  return unavailable();
}
export async function sendOffer(_id: string, _total: number, _reason: string): Promise<Order> {
  return unavailable();
}
export async function createShippingLabel(_id: string): Promise<Order> {
  return unavailable();
}
