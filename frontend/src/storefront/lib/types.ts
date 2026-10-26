import type { PrintModel } from '../../lib/types';

export type PrintSettings = {
  material: string;
  color: string;
  customShade: string;
  quality: string;
  strength: string;
};
export type Model = {
  id: string;
  name: string;
  quantity: number;
  sample?: boolean;
  settings?: PrintSettings;
  analysis?: NonNullable<PrintModel['analysis']>;
  status?: 'analyzing' | 'ready' | 'error';
  error?: string;
  checkoutOptions?: Pick<PrintModel, 'advanced' | 'notes' | 'strength' | 'units'>;
};
export type Material = {
  family?: 'PLA' | 'ABS' | 'PETG' | 'ASA' | 'PA-CF' | 'TPU';
  id: string;
  name: string;
  description: string;
  compatible?: boolean;
  colors: { id: string; name: string; hex: string; available: boolean }[];
};
export type QuoteOptions = {
  models: Model[];
  delivery: string;
  manual: boolean;
  simulateError?: boolean;
};
export type Quote = {
  lines: { id: string; unitPrice: number; subtotal: number }[];
  total: number;
  delivery: number;
  manual: boolean;
  setup?: number;
  reason?: string;
};
