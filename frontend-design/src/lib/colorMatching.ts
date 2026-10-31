import { customColors } from './colors';
import { filamentColors } from './filamentColors';
export const resolveFilament = (value:string) => filamentColors.find(c=>c.id===value);
export function resolveColor(value:string):string|undefined {
  return resolveFilament(value)?.hex ?? (/^#[0-9a-f]{6}$/i.test(value) ? value.toUpperCase() : customColors.find(c=>c.id===value)?.hex.toUpperCase());
}
export const colorsForMaterial = (material:string)=>filamentColors.filter(c=>c.material===material);
export function customColorSummary(material:string,value:string) {
  const filament=resolveFilament(value);
  if(filament && filament.material===material)return `Bambu Lab ${filament.series} · ${filament.name} · kod ${filament.code} · ${filament.hex}${filament.hexes.length>1?' (wielobarwny; podgląd orientacyjny)':''}; potwierdzenie przez e-mail`;
  const hex=resolveColor(value);
  return hex?`${hex} · odcień do indywidualnego doboru przez e-mail`:'Wybierz kolor producenta';
}
