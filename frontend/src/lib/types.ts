export type Role = 'client' | 'admin';
export type Session = { id: string; email: string; name: string; role: Role };
export type DemoScenario = 'normal' | 'empty' | 'error';
export type MaterialColor = { name: string; hex: string; inStock: boolean };
export type Material = {
  id: string;
  name: string;
  description: string;
  colors: MaterialColor[];
  compatible: boolean;
  costPerKg: number;
  priceMultiplier: number;
};
export type QuoteOptions = {
  materialId: string;
  color: string;
  quantity: number;
  quality: 'draft' | 'standard' | 'fine';
};
export type DemoModel = { name: string; dimensions: string; weight: string };
export type PrintModel = DemoModel & {
  id: string;
  format: 'STL' | 'STEP' | '3MF';
  parts: number;
  units: 'mm' | 'cm' | 'in';
  options: QuoteOptions;
  strength: 'light' | 'standard' | 'strong' | 'custom';
  advanced: PrintSettings;
  notes: string;
  analysis?: {
    id: string;
    dimensions_mm: [number, number, number];
    weight_g: number | null;
    weight_source: string;
    reason?: string;
  };
  shipping?: { dimensions_mm: [number, number, number]; weight_g: number };
};
export type PrintSettings = {
  wallCount: number;
  infill: number;
  pattern: 'gyroid' | 'grid';
  orientation: 'auto' | 'flat' | 'upright' | 'custom';
  supports: 'auto' | 'limit_marks' | 'soluble';
  finish: 'standard' | 'sanded' | 'painted';
  fit: 'normal' | 'fit';
  insertCount: number;
  insertSize: 'M3' | 'M4' | 'M5';
  turnaround: 'standard' | 'express';
  drawing: { name: string; size: number } | null;
};
export type Choice<T extends string> = { value: T; label: string; description: string };
export type PrintOptionsCatalog = {
  qualities: Choice<QuoteOptions['quality']>[];
  strengths: (Choice<PrintModel['strength']> & { wallCount: number; infill: number })[];
  patterns: Choice<PrintSettings['pattern']>[];
  orientations: Choice<PrintSettings['orientation']>[];
  supports: Choice<PrintSettings['supports']>[];
  finishes: Choice<PrintSettings['finish']>[];
  fits: Choice<PrintSettings['fit']>[];
  turnarounds: Choice<PrintSettings['turnaround']>[];
  insertSizes: PrintSettings['insertSize'][];
};
export type QuoteLine = {
  modelId: string;
  name: string;
  quantity: number;
  unitPrice: number;
  total: number;
};
export type Quote = {
  id: string;
  version: number;
  fingerprint: string;
  total: number;
  unitPrice: number;
  leadTime: string;
  subtotal: number;
  delivery: number;
  packages: number;
  lines: QuoteLine[];
  manual: boolean;
  reason?: string;
  checkout?: CheckoutQuote;
  deliveryPending?: boolean;
};
export type CheckoutQuote = {
  field_errors?: Partial<Record<keyof Contact, string>>;
  id: string;
  expires_at: number;
  packing: {
    status: 'estimated' | 'manual_review';
    parcels: {
      box_id: string;
      outer_mm: number[];
      weight_g: number;
      placements?: { part_id: string; position_mm: number[]; dimensions_mm: number[] }[];
    }[];
    packaging_grosze: number;
    reason?: string;
  };
  offers: { id: string; name: string; price_grosze: number }[];
  print_grosze: number;
  print_lines: { model_id: string; quantity: number; unit_grosze: number; total_grosze: number }[];
  setup_grosze: number;
  packaging_grosze: number;
  production_demo: boolean;
  stripe_configured: boolean;
  furgonetka_configured: boolean;
  reason?: string;
};
export type DeliveryMethod = 'courier' | 'locker' | 'pickup';
export type PickupPoint = {
  code: string;
  name: string;
  address: { street: string; postcode: string; city: string };
};
export type StripeStatus = {
  status: 'paid' | 'unpaid';
  total_grosze: number;
  livemode: false;
  quote_id: string;
  checkout_status: 'open' | 'complete' | 'expired' | 'processing';
  url: string;
};
export type DeliveryOption = {
  id: DeliveryMethod;
  name: string;
  description: string;
  price: number;
  packages: number;
};
export type Cart = {
  models: PrintModel[];
  delivery: DeliveryMethod;
  postalCode: string;
  quote: Quote | null;
};
export type Contact = {
  name: string;
  email: string;
  phone: string;
  street: string;
  postalCode: string;
  city: string;
  company: string;
  nip: string;
  pickupPoint: string;
};
export type NotificationSettings = {
  message: boolean;
  status: boolean;
  shipped: boolean;
  pickup: boolean;
};
export type Profile = { contact: Contact; notifications: NotificationSettings };
export type PaymentStatus = 'unpaid' | 'paid' | 'pending' | 'cancelled' | 'error' | 'refunded';
export type PaymentScenario = 'success' | 'pending' | 'cancelled' | 'error' | 'reprice';
export type OrderStatus =
  | 'awaiting_payment'
  | 'review'
  | 'preparing'
  | 'printing'
  | 'shipped'
  | 'ready'
  | 'completed'
  | 'cancelled';
export type ChatMessage = {
  id: string;
  sender: Role;
  text: string;
  createdAt: string;
  read: boolean;
};
export type OrderEvent = { at: string; text: string };
export type SandboxPayment = {
  sessionId: string;
  url: string;
  quoteId: string;
  amountGrosze: number;
  offer: CheckoutQuote['offers'][number];
};
export type Order = {
  sandboxPayment?: SandboxPayment;
  id: string;
  number: string;
  createdAt: string;
  ownerId: string | null;
  contact: Contact;
  models: PrintModel[];
  quote: Quote;
  delivery: DeliveryMethod;
  status: OrderStatus;
  payment: PaymentStatus;
  leadTime: string;
  tracking: string;
  label: string;
  document: string;
  notes: string;
  needsAttention: boolean;
  messages: ChatMessage[];
  history: OrderEvent[];
  offerReason: string;
};
export type CheckoutInput = {
  requestOnly?: boolean;
  quoteId: string;
  version: number;
  contact: Contact;
  scenario: PaymentScenario;
};
export type CheckoutResult = { order: Order | null; quote: Quote; repriced: boolean };
export type EmailChallenge = { id: string; email: string; isNew: boolean };
export type SystemJob = {
  id: string;
  kind: 'conversion' | 'quote' | 'email';
  label: string;
  status: 'failed' | 'retried';
};
export type SystemSettings = {
  setupFee: number;
  minimumPrice: number;
  electricityPerKwh: number;
  jobs: SystemJob[];
};
