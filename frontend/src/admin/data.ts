export const stages = ['Nowe', 'Do wyceny', 'Czeka na akceptację', 'Przygotowanie', 'Drukowanie', 'Kontrola jakości', 'Pakowanie', 'Wysłane', 'Zakończone'] as const;
export type Stage = typeof stages[number];
export type Model = { name: string; material: string; color: string; qty: number; quality: string; strength: string; price: number; done: number };
export type Order = { id: string; client: string; email: string; date: string; due: string; status: Stage; paid: boolean; attention: string; delivery: string; address: string; models: Model[]; notes: string[]; history: string[]; message: string; draft: string };
const model = (name: string, material: string, qty: number, price: number, color = 'Czarny', done = 0): Model => ({ name, material, qty, price, color, done, quality: 'Dokładny · 0,12 mm', strength: 'Wzmocniona' });
export function seed(): Order[] {
 const rows: [string,string,Stage,string,string,Model[],boolean,string][] = [
 ['1048','Marta Wiśniewska','Nowe','2026-10-28','Nowe zapytanie',[model('uchwyt-montazowy.stl','PA-CF',4,45),model('dystans.stl','PA-CF',8,12)],false,'Czy uchwyt wytrzyma pracę na zewnątrz? Zależy mi na dostawie do środy.'],
 ['1047','Jakub Nowak','Drukowanie','2026-10-24','Termin dzisiaj',[model('korpus-obudowy.stl','PETG',6,32,'Zielony',4),model('pokrywa.stl','PETG',6,18,'Zielony',2)],true,'Kolor zielony jak w konfiguratorze. Pokrywy bez dodatkowej obróbki.'],
 ['1046','Pracownia Forma','Do wyceny','2026-10-29','Sprawdź grubość ścianek',[model('makieta-segment-a.stl','PLA',3,85,'Biały'),model('makieta-segment-b.stl','PLA',2,64,'Biały')],false,'Czy możecie sprawdzić cienkie elementy przed drukiem?'],
 ['1045','Adam Zieliński','Czeka na akceptację','2026-10-30','',[model('oslona-przewodow.stl','TPU',10,24)],false,'Poproszę o elastyczny materiał.'],
 ['1044','Studio Mechanika','Przygotowanie','2026-10-27','',[model('zebatka.stl','PETG',8,27),model('adapter.stl','PETG',4,19)],true,'Otwór montażowy jest najważniejszym wymiarem.'],
 ['1043','Katarzyna Lis','Pakowanie','2026-10-26','',[model('figurka.stl','PLA',2,72,'Biały',2)],true,'Proszę dobrze zabezpieczyć do transportu.'],
 ['1042','Robotyka Projekt','Kontrola jakości','2026-10-27','',[model('wspornik-silnika.stl','PA-CF',4,56,'Czarny',4)],true,'Sprawdzenie dopasowania przed kolejną serią.'],
 ['1041','Piotr Wójcik','Wysłane','2026-10-23','',[model('obudowa-czujnika.stl','ASA',3,39,'Czarny',3)],true,'Dziękuję za pomoc z doborem materiału.'],
 ];
 return rows.map(([id,client,status,due,attention,models,paid,message],i)=>({id:`RM-${id}`,client,status,due,attention,models,paid,message,email:`klient${i+1}@example.com`,date:`2026-10-${24-Math.floor(i/2)}`,delivery:i===4?'Odbiór osobisty':'Kurier',address:'Przykładowa 12, 05-500 Piaseczno',notes:[],history:['Zapytanie otrzymane przez formularz strony.'],draft:''}));
}
export const total = (o: Order) => o.models.reduce((s,m)=>s+m.price*m.qty,0);
export const pieces = (o: Order) => o.models.reduce((s,m)=>s+m.qty,0);
export const money = (n:number) => new Intl.NumberFormat('pl-PL',{style:'currency',currency:'PLN'}).format(n);

// Prefix dangerous cells inside quotes (OWASP CSV Injection); exported for spreadsheet viewing.
export function csvCell(value: string): string {
 const text = /^[\s\uFEFF]*[=+\-@＝＋－＠]|[\t\r\n]/u.test(value) ? "\t" + value : value;
 return '"' + text.replaceAll('"', '""') + '"';
}
