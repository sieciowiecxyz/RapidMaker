/** Operator requests use the server's HttpOnly session, never a browser-stored token. */
export async function operator<T>(path: string, body?: unknown): Promise<T> {
  const response = await fetch(`/api/admin/${path}`, {
    method: body === undefined ? 'GET' : 'PUT',
    credentials: 'same-origin',
    headers: body === undefined ? undefined : { 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const data = await response.json().catch(() => null);
  if (!response.ok) {
    if (response.status === 401) window.dispatchEvent(new Event('rapidmaker:session-expired'));
    const message = typeof data?.message === 'string' ? data.message : '';
    throw new Error(
      response.status === 401
        ? 'Sesja operatora wygasła. Zaloguj się ponownie.'
        : message.slice(0, 400) || 'Nie udało się zapisać lub pobrać danych. Spróbuj ponownie.',
    );
  }
  return data as T;
}

/** Decimal PLN/percent to integer grosze/basis points without float rounding. */
export function hundredths(value: string, label: string, maximum = 4_294_967_295): number {
  const text = value.trim().replace(',', '.');
  if (!/^\d+(?:\.\d{1,2})?$/.test(text))
    throw new Error(`${label}: podaj liczbę z maksymalnie dwoma miejscami po przecinku.`);
  const [whole, fraction = ''] = text.split('.');
  const result = Number(whole) * 100 + Number(fraction.padEnd(2, '0'));
  if (!Number.isSafeInteger(result) || result > maximum)
    throw new Error(`${label}: wartość jest zbyt duża.`);
  return result;
}

export interface OperatorColor {
  code: string;
  name: string;
  rgba: string[];
  in_stock: boolean;
}
export interface OperatorMaterial {
  id: string;
  name: string;
  density_g_cm3: number;
  cost_net_grosze_per_kg: number | null;
  production_enabled: boolean;
  colors: OperatorColor[];
  source?: { material?: { p1p_04_profiles?: string[]; base_profile?: string } };
}
export interface Rates {
  version: string;
  setup_grosze: number;
  handling_per_plate_grosze: number;
  machine_per_hour_grosze: number;
  energy_per_kwh_grosze: number;
  printer_watts: number;
  loss_bps: number;
  margin_bps: number;
  vat_bps: number;
  minimum_net_grosze: number;
}
