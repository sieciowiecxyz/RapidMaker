import type {
  PrintSettings,
  PrintOptionsCatalog,
  Cart,
  Contact,
  DeliveryMethod,
  DeliveryOption,
  DemoModel,
  DemoScenario,
  EmailChallenge,
  Material,
  Order,
  OrderStatus,
  PrintModel,
  Profile,
  Quote,
  Session,
  SystemSettings,
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
    pattern: 'grid',
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
      const orders = restored.orders.filter((order) => !order.sandboxPayment);
      const purgedOrders = orders.length !== restored.orders.length;
      restored.orders = orders;
      restored.cart.models = restored.cart.models.map(normalize);
      restored.orders.forEach((order) => (order.models = order.models.map(normalize)));
      if (migrated) restored.cart.quote = null;
      if (migrated || purgedOrders) {
        try {
          localStorage.setItem(KEY, JSON.stringify(restored));
        } catch {
          /* Keep the restored cart when browser storage is read-only. */
        }
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
  state.orders = state.orders.filter((order) => !order.sandboxPayment);
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
function invalidate() {
  state.cart.quote = null;
  save();
}
const fingerprint = () =>
  JSON.stringify([state.cart.models, state.cart.delivery, state.cart.postalCode, state.revision]);

export async function getPrintOptions(): Promise<PrintOptionsCatalog> {
  await wait(true);
  return clone(printCatalog);
}
export async function getDemoModel(): Promise<DemoModel> {
  await wait(true);
  return demoModel();
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
// Drafts stay local; the server supplies every payable amount.
export function saveQuote(quote: Quote, cart: Cart): Quote {
  if (
    JSON.stringify([cart.models, cart.delivery, cart.postalCode]) !==
    JSON.stringify([state.cart.models, state.cart.delivery, state.cart.postalCode])
  )
    throw new Error('Koszyk zmienił się podczas wyceny. Przelicz go ponownie.');
  state.cart.quote = { ...quote, fingerprint: fingerprint(), version: state.revision };
  save();
  return clone(state.cart.quote);
}
export function assertCurrentQuote(quote: Quote) {
  if (state.cart.quote?.id !== quote.id || quote.fingerprint !== fingerprint())
    throw new Error('Koszyk zmienił się. Wróć do wyceny.');
}
export function applyCheckoutStatus(status: StripeStatus) {
  const quote = state.cart.quote;
  const checkout = quote?.checkout;
  if (
    status.status === 'paid' &&
    status.livemode === false &&
    checkout &&
    status.quote_id === checkout.id &&
    quote.fingerprint === fingerprint() &&
    checkout.offers.some(
      (offer) =>
        status.total_grosze ===
        checkout.print_grosze + checkout.packaging_grosze + offer.price_grosze,
    )
  ) {
    state.cart = { ...state.cart, models: [], quote: null };
    save();
  }
}
