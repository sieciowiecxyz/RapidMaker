import type {
  PrintSettings,
  PrintOptionsCatalog,
  Cart,
  ChatMessage,
  CheckoutInput,
  CheckoutResult,
  Contact,
  DeliveryMethod,
  DeliveryOption,
  DemoModel,
  DemoScenario,
  EmailChallenge,
  Material,
  Order,
  OrderStatus,
  PaymentScenario,
  PrintModel,
  Profile,
  Quote,
  QuoteOptions,
  Session,
  SystemSettings,
  SandboxPayment,
  StripeStatus,
} from './types';

// Wyłącznie makieta przeglądarkowa. To nie jest uwierzytelnianie ani bezpieczny magazyn danych.
const KEY = 'rapidmaker.frontend.demo.v1';
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
const profile = (email = ''): Profile => ({
  contact: blankContact(email),
  notifications: { message: true, status: true, shipped: true, pickup: true },
});
const id = () => crypto.randomUUID();
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const now = () => new Date().toISOString();
const colors = () => [
  { name: 'Leśny zielony', hex: '#1b8356', inStock: true },
  { name: 'Jasny szary', hex: '#cdd0c9', inStock: true },
  { name: 'Grafitowy', hex: '#454a49', inStock: true },
  { name: 'Pomarańczowy', hex: '#e99147', inStock: false },
  { name: 'Biały', hex: '#f3f1e8', inStock: true },
];
const demoModel = (): DemoModel => ({
  name: 'Uchwyt na słuchawki.stl',
  dimensions: '86 × 62 × 42 mm',
  weight: '32 g',
});
const printCatalog: PrintOptionsCatalog = {
  qualities: [
    {
      value: 'draft',
      label: 'Robocza · 0,28 mm',
      description: 'Szybszy wydruk, bardziej widoczne warstwy.',
    },
    { value: 'standard', label: 'Standard · 0,20 mm', description: 'Równowaga wyglądu i kosztu.' },
    {
      value: 'fine',
      label: 'Dokładna · 0,12 mm',
      description: 'Drobniejsze warstwy, dłuższy wydruk.',
    },
  ],
  strengths: [
    {
      value: 'light',
      label: 'Lekka',
      description: 'Dekoracje · 2 ściany / 10% wypełnienia',
      wallCount: 2,
      infill: 10,
    },
    {
      value: 'standard',
      label: 'Standard',
      description: 'Codzienne użycie · 3 ściany / 20%',
      wallCount: 3,
      infill: 20,
    },
    {
      value: 'strong',
      label: 'Wzmocniona',
      description: 'Więcej materiału · 5 ścian / 40%',
      wallCount: 5,
      infill: 40,
    },
    {
      value: 'custom',
      label: 'Własne ustawienia',
      description: 'Samodzielny wybór ścian i wypełnienia.',
      wallCount: 3,
      infill: 20,
    },
  ],
  patterns: [
    { value: 'gyroid', label: 'Gyroid', description: 'Przestrzenna struktura wypełnienia.' },
    { value: 'grid', label: 'Siatka', description: 'Prosta struktura krzyżowa.' },
  ],
  orientations: [
    {
      value: 'auto',
      label: 'Dobierze zespół',
      description: 'Orientację ustalimy podczas przygotowania.',
    },
    {
      value: 'flat',
      label: 'Preferuję na płasko',
      description: 'Preferencja do sprawdzenia przez zespół.',
    },
    {
      value: 'upright',
      label: 'Preferuję pionowo',
      description: 'Preferencja do sprawdzenia przez zespół.',
    },
    {
      value: 'custom',
      label: 'Według uwag / rysunku',
      description: 'Opisz ważne powierzchnie i kierunek obciążeń.',
    },
  ],
  supports: [
    {
      value: 'auto',
      label: 'Dobierze zespół',
      description: 'Standardowe usunięcie podpór w cenie demo.',
    },
    {
      value: 'limit_marks',
      label: 'Ogranicz ślady podpór',
      description: 'Ważne powierzchnie ustalimy indywidualnie.',
    },
    {
      value: 'soluble',
      label: 'Podpory rozpuszczalne — zapytanie',
      description: 'Dostępność i zgodność materiałów wymagają potwierdzenia.',
    },
  ],
  finishes: [
    {
      value: 'standard',
      label: 'Standard po wydruku',
      description: 'Usunięcie standardowych podpór; warstwy pozostają widoczne.',
    },
    {
      value: 'sanded',
      label: 'Szlifowanie — zapytanie',
      description: 'Zakres i możliwość wykonania do ustalenia.',
    },
    {
      value: 'painted',
      label: 'Malowanie — zapytanie',
      description: 'Kolor, wykończenie i dostępność do ustalenia.',
    },
  ],
  fits: [
    {
      value: 'normal',
      label: 'Bez specjalnego pasowania',
      description: 'Bez deklarowanej dokładności wymiarowej.',
    },
    {
      value: 'fit',
      label: 'Element ma do czegoś pasować',
      description: 'Podaj ważne wymiary i element współpracujący; ustalimy wykonalność.',
    },
  ],
  turnarounds: [
    {
      value: 'standard',
      label: 'Standardowy',
      description: 'Przykładowo 2–3 dni robocze; termin po weryfikacji.',
    },
    {
      value: 'express',
      label: 'Zapytaj o ekspres',
      description: 'Dostępność i dopłata wymagają potwierdzenia.',
    },
  ],
  insertSizes: ['M3', 'M4', 'M5'],
};
const defaults = (strength: PrintModel['strength'] = 'standard'): PrintSettings => {
  const preset =
    printCatalog.strengths.find((item) => item.value === strength) ?? printCatalog.strengths[1];
  return {
    wallCount: preset.wallCount,
    infill: preset.infill,
    pattern: 'gyroid',
    orientation: 'auto',
    supports: 'auto',
    finish: 'standard',
    fit: 'normal',
    insertCount: 0,
    insertSize: 'M3',
    turnaround: 'standard',
    drawing: null,
  };
};
const model = (name = 'Uchwyt na słuchawki.stl'): PrintModel => ({
  ...demoModel(),
  name,
  id: id(),
  format: /\.(stp|step)$/i.test(name) ? 'STEP' : /\.3mf$/i.test(name) ? '3MF' : 'STL',
  parts: 1,
  units: 'mm',
  options: { materialId: 'pla', color: 'Leśny zielony', quantity: 1, quality: 'standard' },
  strength: 'standard',
  advanced: defaults(),
  notes: '',
});
type State = {
  session: Session | null;
  challenge: EmailChallenge | null;
  users: Session[];
  profiles: Record<string, Profile>;
  materials: Material[];
  cart: Cart;
  orders: Order[];
  settings: SystemSettings;
  scenario: DemoScenario;
  revision: number;
};
function seed(): State {
  const users: Session[] = [
    { id: 'demo-client', email: 'client@rapidmaker.test', name: 'Anna Kowalska', role: 'client' },
    { id: 'demo-admin', email: 'admin@rapidmaker.test', name: 'Zespół Rapidmaker', role: 'admin' },
  ];
  const state: State = {
    session: null,
    challenge: null,
    users,
    profiles: {},
    scenario: 'normal',
    revision: 1,
    materials: [
      {
        id: 'pla',
        name: 'Bambu PLA Basic',
        description: 'Uniwersalny. Idealny na początek.',
        colors: colors(),
        compatible: true,
        costPerKg: 89,
        priceMultiplier: 1,
      },
      {
        id: 'petg',
        name: 'Bambu PETG HF',
        description: 'Wytrzymały. Do codziennych zadań.',
        colors: colors().slice(0, 3),
        compatible: true,
        costPerKg: 99,
        priceMultiplier: 1.2,
      },
      {
        id: 'abs',
        name: 'Bambu ABS',
        description: 'Do części technicznych i wymagających projektów.',
        colors: colors().slice(1, 3),
        compatible: true,
        costPerKg: 109,
        priceMultiplier: 1.4,
      },
      {
        id: 'tpu',
        name: 'Bambu TPU 95A HF',
        description: 'Elastyczny materiał. Aktualnie niedostępny na naszych drukarkach.',
        colors: colors().slice(2, 3),
        compatible: false,
        costPerKg: 189,
        priceMultiplier: 1.8,
      },
    ],
    cart: { models: [], delivery: 'courier', postalCode: '', quote: null },
    orders: [],
    settings: {
      setupFee: 12,
      minimumPrice: 24,
      electricityPerKwh: 1.2,
      jobs: [
        {
          id: 'job-1',
          kind: 'conversion',
          label: 'Obudowa.step — konwersja przerwana',
          status: 'failed',
        },
        {
          id: 'job-2',
          kind: 'quote',
          label: 'RM-26042 — wycena wymaga ponowienia',
          status: 'failed',
        },
        { id: 'job-3', kind: 'email', label: 'Powiadomienie o wysyłce RM-26041', status: 'failed' },
      ],
    },
  };
  state.profiles['demo-client'] = {
    ...profile(users[0].email),
    contact: {
      name: 'Anna Kowalska',
      email: users[0].email,
      phone: '501 234 567',
      street: 'Zielona 12/4',
      postalCode: '00-001',
      city: 'Warszawa',
      company: '',
      nip: '',
      pickupPoint: 'WAW01M — ul. Zielona 8',
    },
  };
  state.profiles['demo-admin'] = profile(users[1].email);
  const statuses: OrderStatus[] = ['printing', 'shipped', 'review', 'awaiting_payment', 'ready'];
  statuses.forEach((status, i) => {
    const item = model(
      i === 2
        ? 'Obudowa sterownika.step'
        : i === 4
          ? 'Uchwyt do monitora.3mf'
          : 'Uchwyt na słuchawki.stl',
    );
    item.options.quantity = i + 1;
    const quote: Quote = {
      id: `seed-quote-${i}`,
      version: 1,
      fingerprint: '',
      total: 49 + i * 24,
      unitPrice: 24,
      leadTime: '2–3 dni robocze',
      subtotal: 36 + i * 24,
      delivery: 13,
      packages: 1,
      lines: [
        { modelId: item.id, name: item.name, quantity: i + 1, unitPrice: 24, total: 24 * (i + 1) },
      ],
      manual: status === 'review',
    };
    const createdAt = new Date(Date.now() - (i + 1) * 86400000).toISOString();
    state.orders.push({
      id: `demo-order-${i + 1}`,
      number: `RM-2604${i + 1}`,
      createdAt,
      ownerId: i === 2 ? 'other-client' : 'demo-client',
      contact:
        i === 2
          ? {
              ...state.profiles['demo-client'].contact,
              name: 'Michał Nowak',
              email: 'michal@example.test',
            }
          : clone(state.profiles['demo-client'].contact),
      models: [item],
      quote,
      delivery: i === 4 ? 'pickup' : 'courier',
      status,
      payment: status === 'awaiting_payment' || status === 'review' ? 'unpaid' : 'paid',
      leadTime: '22 października',
      tracking: status === 'shipped' ? 'DEMO-PL-26042' : '',
      label: '',
      document: status === 'review' ? '' : `Dokument sprzedaży ${i + 1}/10/2026 — podgląd demo`,
      notes: i === 0 ? 'Sprawdzić powierzchnię styku przed pakowaniem.' : '',
      needsAttention: status === 'review',
      messages: [
        {
          id: `msg-${i}`,
          sender: 'admin',
          text: 'Dziękujemy za projekt! Wszystkie ustalenia do tego zamówienia znajdziesz w tej rozmowie.',
          createdAt,
          read: false,
        },
      ],
      history: [{ at: createdAt, text: 'Zamówienie przyjęte' }],
      offerReason: '',
    });
  });
  return state;
}
function load(): State {
  try {
    const saved = localStorage.getItem(KEY);
    if (saved) {
      const restored: State = JSON.parse(saved);
      let migrated = false;
      const normalize = (item: PrintModel) => {
        if (!item.advanced) {
          item.advanced = defaults(item.strength);
          migrated = true;
        }
        return item;
      };
      restored.cart.models = restored.cart.models.map(normalize);
      restored.orders.forEach((order) => (order.models = order.models.map(normalize)));
      if (migrated) {
        restored.cart.quote = null;
        localStorage.setItem(KEY, JSON.stringify(restored));
      }
      return restored;
    }
  } catch {
    /* Brak magazynu: demo nadal działa w pamięci. */
  }
  return seed();
}
let state = load();
function save() {
  try {
    localStorage.setItem(KEY, JSON.stringify(state));
  } catch {
    /* Przeglądarka może wyłączyć zapis. */
  }
}
async function wait(read = false) {
  await new Promise((resolve) => setTimeout(resolve, 250));
  if (read && state.scenario === 'error')
    throw new Error('Nie udało się pobrać danych. Spróbuj ponownie.');
}
function session() {
  if (!state.session) throw new Error('Zaloguj się, aby otworzyć ten widok.');
  return state.session;
}
function admin() {
  if (session().role !== 'admin')
    throw new Error('Ten widok jest dostępny dla administratora demo.');
}
function findOrder(orderId: string, guest = false) {
  const order = state.orders.find((item) => item.id === orderId);
  if (!order) throw new Error('Nie znaleziono zamówienia.');
  if (
    state.session?.role === 'admin' ||
    (state.session && order.ownerId === state.session.id) ||
    (guest &&
      !order.ownerId &&
      (state.cart.quote?.id === order.quote.id || hasCheckoutReceipt(order)))
  )
    return order;
  throw new Error('Nie masz dostępu do tego zamówienia w demonstracji.');
}
function hasCheckoutReceipt(order: Order) {
  try {
    const receipt = JSON.parse(sessionStorage.getItem('rapidmaker.last-checkout') ?? 'null');
    return !!order.sandboxPayment && receipt?.sessionId === order.sandboxPayment.sessionId;
  } catch {
    return false;
  }
}
function history(order: Order, text: string) {
  order.history.push({ at: now(), text });
  save();
}
function invalidate() {
  state.cart.quote = null;
  save();
}
const fingerprint = () =>
  JSON.stringify([state.cart.models, state.cart.delivery, state.cart.postalCode, state.revision]);
const round = (n: number) => Math.round(n * 100) / 100;
function validateOptions(options: QuoteOptions) {
  const material = state.materials.find((item) => item.id === options.materialId);
  if (
    !material?.compatible ||
    !material.colors.some((item) => item.name === options.color && item.inStock)
  )
    throw new Error('Wybierz dostępny materiał i kolor.');
  if (!Number.isInteger(options.quantity) || options.quantity < 1 || options.quantity > 1000)
    throw new Error('Wybierz od 1 do 1000 sztuk.');
  return material;
}
const qualityFactor = (quality: QuoteOptions['quality']) =>
  quality === 'draft' ? 0.85 : quality === 'fine' ? 1.25 : 1;
function makeQuote(): Quote {
  const lines = state.cart.models.map((item) => {
    const material = validateOptions(item.options);
    const unitPrice = round(
      24 *
        material.priceMultiplier *
        qualityFactor(item.options.quality) *
        (0.7 + item.advanced.wallCount * 0.06 + item.advanced.infill * 0.006),
    );
    return {
      modelId: item.id,
      name: item.name,
      quantity: item.options.quantity,
      unitPrice,
      total: round(unitPrice * item.options.quantity),
    };
  });
  const count = lines.reduce((sum, line) => sum + line.quantity, 0);
  const packages = 0; // Packing is calculated by the Rust checkout API.
  const manual =
    count > 50 ||
    state.cart.models.some((item) => {
      const a = item.advanced;
      return (
        item.notes.trim().length > 0 ||
        a.orientation !== 'auto' ||
        a.supports !== 'auto' ||
        a.finish !== 'standard' ||
        a.fit !== 'normal' ||
        a.insertCount > 0 ||
        a.turnaround !== 'standard' ||
        !!a.drawing
      );
    });
  const subtotal = Math.max(
    state.settings.minimumPrice,
    round(state.settings.setupFee + lines.reduce((sum, line) => sum + line.total, 0)),
  );
  const delivery = 0; // No fabricated carrier price; API adapter marks missing rates pending.
  return {
    id: id(),
    version: state.revision,
    fingerprint: fingerprint(),
    total: round(subtotal + delivery),
    unitPrice: round(subtotal / Math.max(1, count)),
    leadTime: manual ? 'Po potwierdzeniu wymagań' : '2–3 dni robocze',
    subtotal,
    delivery,
    packages: state.cart.delivery === 'pickup' ? 0 : packages,
    lines,
    manual,
    reason: manual
      ? 'Duża partia lub indywidualne wymagania. Obsługa przygotuje ofertę i potwierdzi dostawę.'
      : undefined,
  };
}

export async function getPrintOptions(): Promise<PrintOptionsCatalog> {
  await wait(true);
  return clone(printCatalog);
}
export async function getMaterials(): Promise<Material[]> {
  await wait(true);
  return state.scenario === 'empty' ? [] : clone(state.materials);
}
export async function getDemoModel(): Promise<DemoModel> {
  await wait(true);
  return demoModel();
}
export async function getQuote(options: QuoteOptions): Promise<Quote> {
  await wait();
  const material = validateOptions(options);
  const total = round(
    state.settings.setupFee +
      24 * material.priceMultiplier * options.quantity * qualityFactor(options.quality),
  );
  return {
    id: id(),
    version: state.revision,
    fingerprint: JSON.stringify(options),
    total,
    unitPrice: total / options.quantity,
    leadTime: '2–3 dni robocze',
    subtotal: total,
    delivery: 0,
    packages: 1,
    lines: [],
    manual: false,
  };
}
export async function getSession(): Promise<Session | null> {
  await wait();
  return clone(state.session);
}
export async function getDemoAccounts(): Promise<{ email: string; role: Session['role'] }[]> {
  return [
    { email: 'client@rapidmaker.test', role: 'client' },
    { email: 'admin@rapidmaker.test', role: 'admin' },
  ];
}
export async function requestLogin(email: string, register = false): Promise<EmailChallenge> {
  await wait();
  email = email.trim().toLowerCase();
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) throw new Error('Podaj poprawny adres email.');
  if (!register && !state.users.some((user) => user.email === email))
    throw new Error('Nie masz jeszcze konta demo. Wybierz utworzenie konta.');
  state.challenge = { id: id(), email, isNew: !state.users.some((user) => user.email === email) };
  save();
  return clone(state.challenge);
}
export async function confirmLogin(challengeId: string, expired = false): Promise<Session> {
  await wait();
  const challenge = state.challenge;
  if (expired || !challenge || challenge.id !== challengeId) {
    state.challenge = null;
    save();
    throw new Error('Link wygasł. Poproś o nowy link demonstracyjny.');
  }
  let user = state.users.find((item) => item.email === challenge.email);
  if (!user) {
    user = {
      id: id(),
      email: challenge.email,
      name: challenge.email.split('@')[0],
      role: 'client',
    };
    state.users.push(user);
    state.profiles[user.id] = profile(user.email);
  }
  state.session = user;
  state.challenge = null;
  state.orders
    .filter((order) => !order.ownerId && order.contact.email.toLowerCase() === user.email)
    .forEach((order) => (order.ownerId = user.id));
  save();
  return clone(user);
}
export async function logout() {
  await wait();
  state.session = null;
  state.challenge = null;
  save();
}
export async function getCart(): Promise<Cart> {
  await wait(true);
  return clone(state.cart);
}
export async function addModel(name: string): Promise<Cart> {
  await wait();
  if (!/\.(stl|step|stp|3mf)$/i.test(name)) throw new Error('Wybierz plik STL, STEP lub 3MF.');
  if (state.cart.models.length >= 10)
    throw new Error('W demonstracji można dodać maksymalnie 10 modeli.');
  state.cart.models.push(model(name));
  invalidate();
  return clone(state.cart);
}
export async function updateModel(modelId: string, patch: Partial<PrintModel>): Promise<Cart> {
  await wait();
  const index = state.cart.models.findIndex((item) => item.id === modelId);
  if (index < 0) throw new Error('Nie znaleziono modelu.');
  const current = state.cart.models[index];
  const advanced = { ...current.advanced, ...patch.advanced };
  if (patch.strength && patch.strength !== 'custom' && patch.strength !== current.strength) {
    const preset = defaults(patch.strength);
    advanced.wallCount = preset.wallCount;
    advanced.infill = preset.infill;
  }
  if (
    !Number.isInteger(advanced.wallCount) ||
    advanced.wallCount < 2 ||
    advanced.wallCount > 8 ||
    !Number.isInteger(advanced.infill) ||
    advanced.infill < 10 ||
    advanced.infill > 100 ||
    !Number.isInteger(advanced.insertCount) ||
    advanced.insertCount < 0 ||
    advanced.insertCount > 100
  )
    throw new Error('Wybierz 2–8 ścian, 10–100% wypełnienia i 0–100 insertów.');
  state.cart.models[index] = { ...current, ...patch, advanced, id: modelId };
  invalidate();
  return clone(state.cart);
}
export async function removeModel(modelId: string): Promise<Cart> {
  await wait();
  state.cart.models = state.cart.models.filter((item) => item.id !== modelId);
  invalidate();
  return clone(state.cart);
}
export async function setDelivery(delivery: DeliveryMethod, postalCode = ''): Promise<Cart> {
  await wait();
  state.cart.delivery = delivery;
  state.cart.postalCode = postalCode;
  invalidate();
  return clone(state.cart);
}
export async function getDeliveryOptions(): Promise<DeliveryOption[]> {
  await wait(true);
  return state.scenario === 'empty'
    ? []
    : [
        {
          id: 'courier',
          name: 'Kurier',
          description: 'Dostawa pod wskazany adres',
          price: 0,
          packages: 1,
        },
        {
          id: 'locker',
          name: 'Paczkomat InPost',
          description: 'Wybierz rzeczywisty punkt z listy',
          price: 0,
          packages: 1,
        },
        {
          id: 'pickup',
          name: 'Odbiór osobisty',
          description: 'Po potwierdzeniu gotowości',
          price: 0,
          packages: 0,
        },
      ];
}
export async function quoteCart(): Promise<Quote> {
  await wait();
  if (!state.cart.models.length) throw new Error('Dodaj co najmniej jeden model.');
  const quote = makeQuote();
  state.cart.quote = quote;
  save();
  return clone(quote);
}
function payment(order: Order, scenario: PaymentScenario) {
  if (scenario === 'reprice')
    throw new Error('Zmianę ceny sprawdzisz przy tworzeniu nowego zamówienia.');
  order.payment = scenario === 'success' ? 'paid' : scenario;
  if (order.payment === 'paid') {
    order.status = 'preparing';
    order.document = `Dokument sprzedaży ${order.number} — demonstracja`;
  }
  history(order, `Symulacja płatności: ${order.payment}`);
}
function createOrder(quote: Quote, contact: Contact, requestOnly = false): Order {
  const order: Order = {
    id: id(),
    number: `RM-${String(26046 + state.orders.length)}`,
    createdAt: now(),
    ownerId: state.session?.role === 'client' ? state.session.id : null,
    contact: clone({ ...contact, email: contact.email.trim().toLowerCase() }),
    models: clone(state.cart.models),
    quote: clone(quote),
    delivery: state.cart.delivery,
    status: quote.manual || requestOnly ? 'review' : 'awaiting_payment',
    payment: 'unpaid',
    leadTime: quote.leadTime,
    tracking: '',
    label: '',
    document: '',
    notes: '',
    needsAttention: quote.manual || !!requestOnly,
    messages: [],
    history: [{ at: now(), text: quote.manual ? 'Prośba o ręczną wycenę' : 'Zamówienie przyjęte' }],
    offerReason: '',
  };
  return order;
}
export function saveSandboxOrder(quote: Quote, contact: Contact, payment: SandboxPayment): Order {
  const existing = state.orders.find(
    (order) => order.sandboxPayment?.sessionId === payment.sessionId,
  );
  if (existing) return visibleOrder(findOrder(existing.id, true));
  if (state.cart.quote?.id !== quote.id || quote.fingerprint !== fingerprint())
    throw new Error('Koszyk zmienił się. Wróć do wyceny.');
  const order = createOrder(quote, contact);
  order.sandboxPayment = clone(payment);
  order.history = [{ at: now(), text: 'Utworzono zamówienie i płatność Stripe test' }];
  state.orders.unshift(order);
  save();
  return visibleOrder(order);
}
export function getCheckoutOrder(sessionId: string): Order | null {
  const order = state.orders.find((order) => order.sandboxPayment?.sessionId === sessionId);
  return order ? visibleOrder(findOrder(order.id, true)) : null;
}
export function applyCheckoutStatus(sessionId: string, status: StripeStatus): Order | null {
  const order = state.orders.find((order) => order.sandboxPayment?.sessionId === sessionId);
  if (!order) return null;
  findOrder(order.id, true);
  if (
    status.livemode !== false ||
    status.quote_id !== order.sandboxPayment!.quoteId ||
    status.total_grosze !== order.sandboxPayment!.amountGrosze
  )
    throw new Error('Potwierdzenie płatności nie pasuje do zamówienia.');
  if (status.status === 'paid' && order.payment !== 'paid') {
    order.payment = 'paid';
    if (order.status === 'awaiting_payment') order.status = 'preparing';
    if (state.cart.quote?.id === order.quote.id && fingerprint() === order.quote.fingerprint) {
      state.cart = { ...state.cart, models: [], quote: null };
    }
    history(order, 'Stripe potwierdził płatność testową');
  }
  return visibleOrder(order);
}
export async function checkout(input: CheckoutInput): Promise<CheckoutResult> {
  await wait();
  const quote = state.cart.quote;
  if (
    !quote ||
    quote.id !== input.quoteId ||
    quote.version !== input.version ||
    quote.fingerprint !== fingerprint()
  )
    throw new Error('Wycena jest nieaktualna. Wróć do konfiguratora i przelicz cenę.');
  if (!input.contact.name.trim() || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(input.contact.email))
    throw new Error('Uzupełnij imię i nazwisko oraz poprawny email.');
  if (
    state.cart.delivery !== 'pickup' &&
    ((state.cart.delivery === 'courier' && !input.contact.street.trim()) ||
      !input.contact.city.trim() ||
      !/^\d{2}-\d{3}$/.test(input.contact.postalCode))
  )
    throw new Error('Uzupełnij adres dostawy i kod pocztowy w formacie 00-000.');
  if (state.cart.delivery === 'locker' && !input.contact.pickupPoint.trim())
    throw new Error('Wybierz punkt odbioru.');
  const existing = state.orders.find((order) => order.quote.id === quote.id);
  if (existing)
    return { order: clone(findOrder(existing.id, true)), quote: clone(quote), repriced: false };
  if (input.scenario === 'reprice') {
    state.revision++;
    const updated = makeQuote();
    updated.total = round(updated.total + 5);
    updated.subtotal = round(updated.subtotal + 5);
    state.cart.quote = updated;
    save();
    return { order: null, quote: clone(updated), repriced: true };
  }
  const order = createOrder(quote, input.contact, input.requestOnly);
  if (input.requestOnly) order.quote.manual = true;
  if (!quote.manual && !input.requestOnly) payment(order, input.scenario);
  state.orders.unshift(order);
  save();
  return { order: clone(order), quote: clone(quote), repriced: false };
}
export async function retryPayment(orderId: string, scenario: PaymentScenario): Promise<Order> {
  await wait();
  const order = findOrder(orderId, true);
  if (order.sandboxPayment) throw new Error('Sprawdź płatność bezpośrednio w Stripe.');
  if (order.quote.manual) throw new Error('Najpierw zaakceptuj ofertę obsługi.');
  if (order.payment === 'paid' || order.payment === 'refunded' || order.status === 'cancelled')
    throw new Error('To zamówienie nie wymaga ponowienia płatności.');
  payment(order, scenario);
  return visibleOrder(order);
}
function visibleOrder(order: Order): Order {
  const result = clone(order);
  if (state.session?.role !== 'admin') result.notes = '';
  return result;
}
export async function getOrders(): Promise<Order[]> {
  await wait(true);
  const user = session();
  return state.scenario === 'empty'
    ? []
    : state.orders
        .filter((order) => user.role === 'admin' || order.ownerId === user.id)
        .map(visibleOrder);
}
export async function getOrder(orderId: string): Promise<Order> {
  await wait(true);
  return visibleOrder(findOrder(orderId));
}
export async function reorder(orderId: string): Promise<Cart> {
  await wait();
  const order = findOrder(orderId);
  state.cart = {
    models: order.models.map((item) => ({ ...clone(item), id: id() })),
    delivery: order.delivery,
    postalCode: order.contact.postalCode,
    quote: null,
  };
  save();
  return clone(state.cart);
}
export async function sendMessage(orderId: string, text: string): Promise<Order> {
  await wait();
  const user = session();
  const order = findOrder(orderId);
  text = text.trim();
  if (!text) throw new Error('Wpisz wiadomość.');
  if (text.length > 4000) throw new Error('Wiadomość może mieć maksymalnie 4000 znaków.');
  const message: ChatMessage = { id: id(), sender: user.role, text, createdAt: now(), read: false };
  order.messages.push(message);
  order.needsAttention = user.role === 'client';
  save();
  return visibleOrder(order);
}
export async function markMessagesRead(orderId: string): Promise<Order> {
  await wait();
  const user = session();
  const order = findOrder(orderId);
  order.messages
    .filter((message) => message.sender !== user.role)
    .forEach((message) => (message.read = true));
  save();
  return visibleOrder(order);
}
export async function getProfile(): Promise<Profile> {
  await wait(true);
  return clone(state.profiles[session().id]);
}
export async function updateProfile(value: Profile): Promise<Profile> {
  await wait();
  const user = session();
  if (value.contact.email.toLowerCase() !== user.email)
    throw new Error('W demonstracji adres logowania pozostaje bez zmian.');
  state.profiles[user.id] = clone(value);
  user.name = value.contact.name || user.name;
  save();
  return clone(value);
}
export async function deleteAccount(): Promise<void> {
  await wait();
  const user = session();
  if (user.role === 'admin') throw new Error('Konta administratora demo nie można usunąć.');
  state.users = state.users.filter((item) => item.id !== user.id);
  delete state.profiles[user.id];
  state.orders
    .filter((order) => order.ownerId === user.id)
    .forEach((order) => (order.ownerId = null));
  state.session = null;
  save();
}
export async function updateOrder(
  orderId: string,
  patch: Partial<Pick<Order, 'status' | 'leadTime' | 'notes' | 'needsAttention'>>,
): Promise<Order> {
  await wait();
  admin();
  const order = findOrder(orderId);
  if (
    patch.status &&
    ['preparing', 'printing', 'shipped', 'ready', 'completed'].includes(patch.status) &&
    order.payment !== 'paid'
  )
    throw new Error('Realizację można rozpocząć dopiero po opłaceniu zamówienia.');
  Object.assign(order, patch);
  history(
    order,
    patch.status ? `Zmieniono status: ${patch.status}` : 'Zaktualizowano dane produkcji',
  );
  return clone(order);
}
export async function sendOffer(orderId: string, total: number, reason: string): Promise<Order> {
  await wait();
  admin();
  if (!reason.trim() || !Number.isFinite(total) || total <= 0)
    throw new Error('Podaj cenę większą od zera i powód zmiany.');
  const order = findOrder(orderId);
  if (order.sandboxPayment) throw new Error('Ta operacja nie jest podłączona do sandboxa.');
  if (['paid', 'refunded'].includes(order.payment) || order.status === 'cancelled')
    throw new Error('Nową ofertę można przygotować dla aktywnego, nieopłaconego zlecenia.');
  if (total < order.quote.delivery)
    throw new Error('Cena całkowita nie może być niższa od dostawy.');
  order.quote.total = round(total);
  order.quote.subtotal = round(total - order.quote.delivery);
  order.quote.manual = false;
  order.quote.version++;
  order.offerReason = reason.trim();
  order.status = 'awaiting_payment';
  order.needsAttention = false;
  history(order, `Wysłano ofertę demo: ${total.toFixed(2)} zł. ${reason}`);
  return clone(order);
}
export async function refundOrder(orderId: string): Promise<Order> {
  await wait();
  admin();
  const order = findOrder(orderId);
  if (order.sandboxPayment) throw new Error('Ta operacja nie jest podłączona do sandboxa.');
  if (order.payment !== 'paid') throw new Error('Zwrócić można tylko opłacone zamówienie.');
  order.payment = 'refunded';
  order.status = 'cancelled';
  history(order, 'Symulacja zwrotu płatności');
  return clone(order);
}
export async function createShippingLabel(orderId: string): Promise<Order> {
  await wait();
  admin();
  const order = findOrder(orderId);
  if (order.sandboxPayment) throw new Error('Ta operacja nie jest podłączona do sandboxa.');
  if (order.delivery === 'pickup') throw new Error('Odbiór osobisty nie wymaga etykiety.');
  if (order.payment !== 'paid') throw new Error('Najpierw potwierdź opłacenie zamówienia.');
  if (!order.label) {
    order.label = `ETYKIETA-DEMO-${order.number}`;
    order.tracking = `DEMO-PL-${order.number}`;
    history(order, 'Utworzono przykładową etykietę wysyłkową');
  }
  return clone(order);
}
export async function updateMaterial(
  materialId: string,
  patch: Partial<Material>,
): Promise<Material[]> {
  await wait();
  admin();
  const item = state.materials.find((item) => item.id === materialId);
  if (!item) throw new Error('Nie znaleziono materiału.');
  if (patch.costPerKg !== undefined && (!Number.isFinite(patch.costPerKg) || patch.costPerKg <= 0))
    throw new Error('Koszt musi być większy od zera.');
  if (
    patch.priceMultiplier !== undefined &&
    (!Number.isFinite(patch.priceMultiplier) || patch.priceMultiplier <= 0)
  )
    throw new Error('Mnożnik musi być większy od zera.');
  Object.assign(item, patch, { id: materialId });
  state.revision++;
  invalidate();
  return clone(state.materials);
}
export async function getSystemSettings(): Promise<SystemSettings> {
  await wait(true);
  admin();
  return clone(state.settings);
}
export async function updateSystemSettings(
  patch: Partial<Pick<SystemSettings, 'setupFee' | 'minimumPrice' | 'electricityPerKwh'>>,
): Promise<SystemSettings> {
  await wait();
  admin();
  if (Object.values(patch).some((value) => !Number.isFinite(value) || value < 0))
    throw new Error('Podaj poprawne wartości nieujemne.');
  Object.assign(state.settings, patch);
  state.revision++;
  invalidate();
  return clone(state.settings);
}
export async function retryJob(jobId: string): Promise<SystemSettings> {
  await wait();
  admin();
  const job = state.settings.jobs.find((item) => item.id === jobId);
  if (!job) throw new Error('Nie znaleziono zadania.');
  job.status = 'retried';
  save();
  return clone(state.settings);
}
export async function getDemoScenario(): Promise<DemoScenario> {
  return state.scenario;
}
export async function setDemoScenario(scenario: DemoScenario): Promise<void> {
  state.scenario = scenario;
  save();
}
export async function resetDemo(): Promise<void> {
  state = seed();
  save();
}
