export type PrintSettings = { material:string; color:string; customShade:string; quality:string; strength:string };
export type Model = { id: string; name: string; quantity: number; sample?: boolean; settings?: PrintSettings };
export type Material = { id: string; name: string; description: string; colors: { id: string; name: string; hex: string; available: boolean }[] };
export type QuoteOptions = { models: Model[]; delivery: string; manual: boolean; simulateError?: boolean };
export type Quote = { lines: { id: string; unitPrice: number; subtotal: number }[]; total: number; delivery: number; manual: boolean };
