import { bambuPlaColors } from './bambuPlaColors';
// Approximate customer preferences, not a claim of filament stock or calibrated color.
const legacyColors = [
  { id:'red', name:'Czerwony', hex:'#d94a42' },
  { id:'orange', name:'Pomarańczowy', hex:'#e39139' },
  { id:'yellow', name:'Żółty', hex:'#e2c347' },
  { id:'blue', name:'Niebieski', hex:'#3979cf' },
  { id:'navy', name:'Granatowy', hex:'#243e78' },
  { id:'purple', name:'Fioletowy', hex:'#8059b5' },
  { id:'pink', name:'Różowy', hex:'#ce6b9c' },
  { id:'brown', name:'Brązowy', hex:'#855537' },
  { id:'gray', name:'Szary', hex:'#858c89' },
  { id:'cream', name:'Kremowy', hex:'#ded3b5' }
];

export const customColors = [...legacyColors.map(c=>({...c,hexes:[c.hex],series:'Orientacyjne',code:''})), ...bambuPlaColors];
