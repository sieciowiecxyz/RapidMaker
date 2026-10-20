import { customColors } from './colors';
import type { Material, Model, PrintSettings } from './types';
export const defaultSettings: PrintSettings = { material:'pla', color:'black', customShade:'blue', quality:'standard', strength:'standard' };
export const emptySettings: PrintSettings = { material:'', color:'', customShade:'', quality:'', strength:'' };
export function normalizeSettings(settings: PrintSettings): PrintSettings {
  return { ...settings, color:settings.material === 'pa' && settings.color !== 'black' ? '' : settings.color };
}
export function validateModel(model: Model, materials: Material[]): string[] {
  const errors:string[]=[];
  if (!Number.isInteger(model.quantity) || model.quantity < 1 || model.quantity > 999) errors.push('liczba sztuk od 1 do 999');
  const settings=model.settings;
  if (!settings) return [...errors,'materiał, kolor, jakość i wytrzymałość'];
  const material=materials.find(item=>item.id===settings.material);
  if (!material) errors.push('materiał');
  if (!(material ?? materials[0])?.colors.some(item=>item.id===settings.color && item.available)) errors.push('dostępny kolor');
  if (settings.color==='custom' && !customColors.some(item=>item.id===settings.customShade)) errors.push('odcień niestandardowy');
  if (!['standard','fine'].includes(settings.quality)) errors.push('jakość');
  if (!['light','standard','reinforced'].includes(settings.strength)) errors.push('wytrzymałość');
  return errors;
}
export function settingsSummary(settings: PrintSettings, materials: Material[]): string {
  if(!settings.material && !settings.color && !settings.quality && !settings.strength) return 'Wybierz ustawienia wydruku';
  const material=materials.find(item=>item.id===settings.material);
  const shade=customColors.find(item=>item.id===settings.customShade);
  const color=settings.color==='custom' ? `${shade?.name ?? 'Wybierz odcień'} (${shade?.hex ?? '?'})` : material?.colors.find(item=>item.id===settings.color)?.name;
  return `${material?.name ?? 'Wybierz materiał'} · ${color ?? 'Wybierz kolor'} · ${settings.quality==='fine'?'Dokładny':settings.quality==='standard'?'Standard':'Wybierz jakość'} · ${{light:'Lekka',standard:'Standard',reinforced:'Wzmocniona'}[settings.strength] ?? 'Wybierz wytrzymałość'}`;
}
