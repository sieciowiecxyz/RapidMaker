import type { OrderStatus, PaymentStatus, Quote } from './types';
export const quoteAdjustment = (quote: Quote) =>
  Math.round((quote.subtotal - quote.lines.reduce((sum, line) => sum + line.total, 0)) * 100) / 100;
export const money = (value: number | null) =>
  value === null
    ? 'Do wyceny'
    : new Intl.NumberFormat('pl-PL', { style: 'currency', currency: 'PLN' }).format(value);
export const date = (value: string) =>
  new Intl.DateTimeFormat('pl-PL', { day: 'numeric', month: 'short', year: 'numeric' }).format(
    new Date(value),
  );
export const orderStatus: Record<OrderStatus, string> = {
  awaiting_payment: 'Oczekuje na płatność',
  review: 'Ręczna wycena',
  quote_expired: 'Wycena wygasła',
  preparing: 'Przygotowanie',
  printing: 'W druku',
  shipped: 'Wysłane',
  ready: 'Gotowe do odbioru',
  completed: 'Zakończone',
  cancelled: 'Anulowane',
};
export const paymentStatus: Record<PaymentStatus, string> = {
  unpaid: 'Nieopłacone',
  paid: 'Opłacone',
  pending: 'Oczekuje na potwierdzenie',
  cancelled: 'Płatność anulowana',
  error: 'Błąd płatności',
  refunded: 'Zwrócono płatność',
};
export const parcels = (count: number) =>
  count === 0
    ? 'Odbiór osobisty'
    : `${count} ${count === 1 ? 'paczka' : count % 10 >= 2 && count % 10 <= 4 && (count % 100 < 12 || count % 100 > 14) ? 'paczki' : 'paczek'}`;
export function printConfiguration(model: import('./types').PrintModel): string[] {
  const a = model.advanced;
  return [
    `Warstwa: ${{ draft: '0,28 mm · robocza', standard: '0,20 mm · standard', fine: '0,12 mm · dokładna' }[model.options.quality]}`,
    `Ściany: ${a.wallCount} · wypełnienie: ${a.infill}% · ${a.pattern === 'gyroid' ? 'gyroid' : 'siatka'}`,
    `Orientacja: ${{ auto: 'dobierze zespół', flat: 'preferowana na płasko', upright: 'preferowana pionowa', custom: 'według uwag / rysunku' }[a.orientation]}`,
    `Podpory: ${{ auto: 'dobierze zespół', limit_marks: 'ograniczenie śladów — zapytanie', soluble: 'rozpuszczalne — zapytanie' }[a.supports]}`,
    `Wykończenie: ${{ standard: 'standard po wydruku', sanded: 'szlifowanie — zapytanie', painted: 'malowanie — zapytanie' }[a.finish]}`,
    `Pasowanie: ${a.fit === 'normal' ? 'bez specjalnego wymagania' : 'do ustalenia z zespołem'}`,
    `Termin: ${a.turnaround === 'standard' ? 'standardowy' : 'ekspres — zapytanie'}`,
    ...(a.insertCount ? [`Inserty: ${a.insertCount} × ${a.insertSize} — zapytanie`] : []),
    ...(model.format === 'STL' ? [`Jednostki STL: ${model.units}`] : []),
    ...(a.drawing ? [`Rysunek: ${a.drawing.name} · tylko nazwa pliku, bez zawartości`] : []),
  ];
}
