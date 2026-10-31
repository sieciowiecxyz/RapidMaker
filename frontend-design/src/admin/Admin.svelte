<script lang="ts">
 import { onMount, tick } from 'svelte';
 import { IconLayoutList, IconCube, IconMessage, IconArrowUpRight, IconSearch, IconX, IconChevronRight, IconArrowRight, IconCheck, IconClock, IconPackage, IconArrowLeft, IconSun, IconMoon, IconAlertCircle, IconDownload, IconCreditCard, IconFileInvoice } from '@tabler/icons-svelte';
 import Finance, { type InvoiceDraft } from './Finance.svelte';
 import { seed, stages, total, pieces, money, type Order, type Stage } from './data';
 let attachments = $state<Record<string, File>>({});
 let fileError = $state('');
 function attach(key:string, event:Event){const input=event.currentTarget as HTMLInputElement;const file=input.files?.[0];if(!file)return;if(!/\.(stl|step|stp|3mf)$/i.test(file.name)){fileError='Wybierz plik STL, STEP lub 3MF.';return;}if(file.size>25*1024*1024){fileError='Maksymalny rozmiar pliku to 25 MB.';return;}attachments[key]=file;fileError='';input.value='';}
 function downloadFile(file:File){const url=URL.createObjectURL(file);const a=document.createElement('a');a.href=url;a.download=file.name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
 let orders = $state<Order[]>(seed());
 let invoices = $state<Record<string,InvoiceDraft>>({});
 let view = $state('Zamówienia');
 let filter = $state('Wszystkie');
 let query = $state('');
 let selectedId = $state<string|null>(null);
 let tab = $state('Modele i wycena');
 let note = $state('');
 let feedback = $state('');
 let storageError = $state('');
 let light = $state(false);
 let panel = $state<HTMLElement>();
 let opener: HTMLElement | null = null;
 let selected = $derived(orders.find(o=>o.id===selectedId));
 let actionCount = $derived(orders.filter(o=>o.attention).length);
 let visible = $derived(orders.filter(o=>[o.id,o.client,...o.models.map(m=>m.name)].join(' ').toLocaleLowerCase('pl').includes(query.toLocaleLowerCase('pl'))).filter(o=>filter==='Wszystkie'||filter==='Wymaga reakcji'&&!!o.attention||filter==='Nowe'&&['Nowe','Do wyceny'].includes(o.status)||filter==='W produkcji'&&['Przygotowanie','Drukowanie','Kontrola jakości'].includes(o.status)||filter==='Do wysłania'&&o.status==='Pakowanie'));
 const production: Stage[] = ['Przygotowanie','Drukowanie','Kontrola jakości','Pakowanie'];
 const statusTone = (status: Stage) => ({'Nowe':'new','Do wyceny':'quote','Czeka na akceptację':'waiting','Przygotowanie':'preparing','Drukowanie':'printing','Kontrola jakości':'quality','Pakowanie':'packing','Wysłane':'sent','Zakończone':'complete'}[status]);
 const swatch = (color:string) => ({Czarny:'#17191b', Biały:'#f3f1e7', Zielony:'#427c58'}[color] ?? '#858e88');
 const fmt = (d:string)=>new Intl.DateTimeFormat('pl-PL',{day:'numeric',month:'short'}).format(new Date(d+'T12:00:00'));
 onMount(()=>{try { const drafts=localStorage.getItem('rapidmaker-admin-invoices-v1');if(drafts){const data=JSON.parse(drafts);if(data&&typeof data==='object'&&!Array.isArray(data)&&Object.values(data).every((d:any)=>d&&typeof d.orderId==='string'&&Array.isArray(d.lines)))invoices=data;}  const stored=localStorage.getItem('rapidmaker-admin-demo-v1'); if(stored){const parsed=JSON.parse(stored); if(Array.isArray(parsed)&&parsed.every(o=>typeof o.id==='string'&&Array.isArray(o.models)&&Array.isArray(o.history)&&Array.isArray(o.notes)&&stages.includes(o.status)))orders=parsed;} }catch{storageError='Nie udało się odczytać zapisanej próbki. Pokazujemy dane początkowe.';} });
 function save(){try{localStorage.setItem('rapidmaker-admin-invoices-v1',JSON.stringify(invoices));localStorage.setItem('rapidmaker-admin-demo-v1',JSON.stringify(orders));}catch{storageError='Przeglądarka nie pozwala zapisać zmian. Pozostaną dostępne do odświeżenia.';}}
 async function open(o:Order,event?:MouseEvent){opener=event?.currentTarget as HTMLElement??null; selectedId=o.id;tab='Modele i wycena';note='';feedback='';await tick();panel?.focus();}
 function payment(o:Order){o.paid=!o.paid;o.history.unshift(o.paid?'Ręcznie oznaczono płatność jako otrzymaną (demo).':'Cofnięto oznaczenie płatności (demo).');save();}
 async function openFinance(o:Order,e?:MouseEvent){await open(o,e);tab='Rozliczenie';}
 function close(){selectedId=null;opener?.focus();}
 function updateStatus(o:Order,status:Stage){o.status=status;o.history.unshift(`Status zmieniony na „${status}”.`);o.attention='';save();feedback='Status zapisany w próbce.';}
 function advance(o:Order){const i=stages.indexOf(o.status);if(i<stages.length-1)updateStatus(o,stages[i+1]);}
 function retreat(o:Order){const i=stages.indexOf(o.status);if(i>0)updateStatus(o,stages[i-1]);}
 function addNote(){if(!selected||!note.trim())return;selected.notes.unshift(note.trim());selected.history.unshift('Dodano notatkę wewnętrzną.');note='';save();feedback='Notatka zapisana.';}
 function exportCsv(){const lines=[['Zamówienie','Klient','Status','Płatność','Termin','Wartość'],...visible.map(o=>[o.id,o.client,o.status,o.paid?'Opłacone':'Nieopłacone',o.due,String(total(o))])];const url=URL.createObjectURL(new Blob(['\uFEFF'+lines.map(r=>r.map(v=>'"'+v.replaceAll('"','""')+'"').join(';')).join('\n')],{type:'text/csv;charset=utf-8'}));const a=document.createElement('a');a.href=url;a.download='rapidmaker-zamowienia-demo.csv';a.click();URL.revokeObjectURL(url);}
 function theme(){light=!light;document.documentElement.dataset.theme=light?'light':'dark';}
 function keyboard(e:KeyboardEvent){if(e.key==='Escape'&&selected)close();}
</script>
<svelte:window onkeydown={keyboard}/>
<svelte:head><title>{view} · RapidMaker Admin</title></svelte:head>
<div class="admin-shell">
 <aside class="sidebar">
  <a class="brand" href="/"><img src="/logo.svg" alt=""/>RapidMaker<span>Panel operatora</span></a>
  <nav aria-label="Panel administratora">
   {#each [{name:'Zamówienia',icon:IconLayoutList},{name:'Produkcja',icon:IconCube},{name:'Wiadomości',icon:IconMessage},{name:'Płatności',icon:IconCreditCard},{name:'Faktury',icon:IconFileInvoice}] as item}
    <button class:active={view===item.name} onclick={()=>{view=item.name;selectedId=null;}}><item.icon size={19}/>{item.name}{#if item.name==='Zamówienia'}<span class="nav-count">{orders.length}</span>{/if}</button>
   {/each}
  </nav>
  <div class="side-bottom"><a href="/" target="_blank" rel="noreferrer">Otwórz stronę <IconArrowUpRight size={17}/></a><div class="operator"><span class="avatar">RM</span><div><strong>RapidMaker</strong><small>Administrator</small></div></div></div>
 </aside>
 <main>
  <header class="topbar"><span>Pracownia <span class="slash">/</span> {view}</span><div><span class="demo-label">Dane demonstracyjne</span><button class="icon" onclick={theme} aria-label={light?'Włącz ciemny motyw':'Włącz jasny motyw'}>{#if light}<IconMoon size={18}/>{:else}<IconSun size={18}/>{/if}</button></div></header>
  <div class="workspace">
   <div class="page-heading"><div><h1>{view}</h1><p>{view==='Zamówienia'?'Od pierwszego pliku do gotowej części. Wszystko w jednym miejscu.':view==='Płatności'?'Rozliczenia zamówień. Kwoty i płatności pod kontrolą.':view==='Faktury'?'Dokumenty powiązane z zamówieniami, od szkicu do wystawienia.':view==='Produkcja'?'Kolejka pracy i postęp realizacji poszczególnych modeli.':'Zapytania klientów przypisane do konkretnych zamówień.'}</p></div>{#if view==='Zamówienia'}<button class="secondary" onclick={exportCsv}><IconDownload size={17}/>Eksport CSV</button>{/if}</div>
   {#if storageError}<p class="warning" role="alert">{storageError}</p>{/if}
   <div class="notice"><IconAlertCircle size={18}/><span>Próbka panelu. Zmiany zapisują się w tej przeglądarce. Zamówienia i wiadomości nie są połączone ze stroną.</span></div>
   {#if view==='Zamówienia'}
    <div class="attention-strip"><div><span class="signal"></span><strong>Wymaga reakcji: {actionCount}</strong><span class="strip-detail">Wyceny, nowe zapytania i pilne terminy.</span></div><button class="text-button" onclick={()=>filter='Wymaga reakcji'}>Sprawdź kolejkę <IconArrowRight size={17}/></button></div>
    <div class="list-tools"><div class="filters" aria-label="Filtr zamówień">{#each ['Wszystkie','Wymaga reakcji','Nowe','W produkcji','Do wysłania'] as f}<button class:chosen={filter===f} aria-pressed={filter===f} onclick={()=>filter=f}>{f}{#if f==='Wszystkie'}<span>{orders.length}</span>{/if}</button>{/each}</div><label class="search"><IconSearch size={18}/><input bind:value={query} placeholder="Szukaj numeru, klienta, pliku…" aria-label="Szukaj zamówień"/>{#if query}<button class="icon" onclick={()=>query=''} aria-label="Wyczyść wyszukiwanie"><IconX size={16}/></button>{/if}</label></div>
    <div class="table-scroll"><table><thead><tr><th>Zamówienie / klient</th><th>Modele</th><th>Realizacja</th><th>Płatność</th><th>Termin</th><th class="amount">Wartość</th><th><span class="sr-only">Szczegóły</span></th></tr></thead><tbody>
     {#each visible as o}<tr class:selected={selectedId===o.id}><td><button class="order-link" onclick={(e)=>open(o,e)}>{o.id}<IconArrowUpRight size={14}/></button><strong class="client">{o.client}</strong><small>{fmt(o.date)} · formularz strony</small></td><td><strong>{o.models.length} {o.models.length===1?'model':'modele'} <span class="muted">/ {pieces(o)} szt.</span></strong><div class="material-colors">{#each [...new Set(o.models.map(m=>m.material))] as material}<span class="material-tag">{material}</span>{/each}{#each [...new Set(o.models.map(m=>m.color))] as color}<span class="color-label"><span class="color-swatch" style:background={swatch(color)} aria-hidden="true"></span>{color}</span>{/each}</div></td><td><span class="status" data-status={statusTone(o.status)} class:printing={o.status==='Drukowanie'} class:new-order={o.status==='Nowe'}>{#if o.status==='Drukowanie'}<span class="signal"></span>{/if}{o.status}</span>{#if o.attention}<small class="attention">{o.attention}</small>{/if}</td><td><span class="payment" class:paid={o.paid}>{#if o.paid}<IconCheck size={14}/>{:else}<IconClock size={14}/>{/if}{o.paid?'Opłacone':'Nieopłacone'}</span></td><td><strong class:urgent={o.due==='2026-10-24'}>{fmt(o.due)}</strong><small>{o.delivery}</small></td><td class="amount">{money(total(o))}</td><td><button class="icon" onclick={(e)=>open(o,e)} aria-label={`Otwórz ${o.id}`}><IconChevronRight size={18}/></button></td></tr>{/each}
    </tbody></table>{#if !visible.length}<div class="empty"><IconSearch size={28}/><h2>Brak pasujących zamówień</h2><p>Zmień frazę lub pokaż wszystkie zamówienia.</p><button class="secondary" onclick={()=>{query='';filter='Wszystkie';}}>Wyczyść filtry</button></div>{/if}</div>
    <footer class="list-footer"><span>{visible.length} z {orders.length} zamówień</span><span>Kwoty demonstracyjne · PLN</span></footer>
   {:else if view==='Produkcja'}
    <div class="board">{#each production as stage}<section class="lane"><h2><span class="status" data-status={statusTone(stage)}>{stage}</span><span>{orders.filter(o=>o.status===stage).length}</span></h2>{#each orders.filter(o=>o.status===stage) as o}<article class="production-card"><button class="order-link" onclick={(e)=>open(o,e)}>{o.id}<IconArrowUpRight size={15}/></button><h3>{o.client}</h3><p>{[...new Set(o.models.map(m=>m.material))].join(', ')} · {pieces(o)} szt.</p><div class="production-progress"><span>{o.models.reduce((s,m)=>s+m.done,0)} / {pieces(o)} gotowych</span><span>{fmt(o.due)}</span></div><progress value={o.models.reduce((s,m)=>s+m.done,0)} max={pieces(o)}></progress><button class="secondary" onclick={()=>advance(o)}>{stage==='Pakowanie'?'Oznacz jako wysłane':'Dalej: '+stages[stages.indexOf(stage)+1]}<IconArrowRight size={15}/></button><button class="production-back" onclick={()=>retreat(o)} title={'Cofnij do: '+stages[stages.indexOf(stage)-1]} aria-label={'Cofnij '+o.id+' do: '+stages[stages.indexOf(stage)-1]}><IconArrowLeft size={13}/>Cofnij etap</button></article>{:else}<p class="lane-empty">Brak zleceń na tym etapie.</p>{/each}</section>{/each}</div>
   {:else if view==='Płatności'||view==='Faktury'}
    {#key view}<Finance {orders} {invoices} mode={view} onopen={openFinance} onsave={save} onpayment={payment}/>{/key}
   {:else}
    <div class="inbox">{#each orders as o}<button class="message-row" onclick={async(e)=>{await open(o,e);tab='Wiadomości';}}><span class="avatar">{o.client.split(' ').map(w=>w[0]).slice(0,2).join('')}</span><span><strong>{o.client} <small>{o.id}</small></strong><span>{o.message}</span></span><IconChevronRight size={20}/></button>{/each}</div>
   {/if}
  </div>
 </main>
 {#if selected}
 <aside class="detail" aria-label={`Szczegóły ${selected.id}`} tabindex="-1" bind:this={panel}>
  <div class="detail-header"><div><span class="muted">Zamówienie</span><h2>{selected.id}</h2></div><button class="icon" aria-label="Zamknij szczegóły" onclick={close}><IconX size={21}/></button></div>
  <div class="detail-person"><h3>{selected.client}</h3><span>{selected.email}</span><small>Dane przykładowe · złożono {fmt(selected.date)}</small></div>
  <div class="detail-summary"><label>Etap realizacji<select value={selected.status} onchange={(e)=>updateStatus(selected!,e.currentTarget.value as Stage)}>{#each stages as s}<option>{s}</option>{/each}</select></label><label>Termin realizacji<input type="date" bind:value={selected.due} onchange={()=>save()}/></label></div>
  <div class="detail-tabs" aria-label="Sekcje zamówienia">{#each ['Modele i wycena','Pliki','Rozliczenie','Wiadomości','Notatki','Historia'] as t}<button class:chosen={tab===t} aria-pressed={tab===t} onclick={()=>tab=t}>{t}</button>{/each}</div>
  <div class="detail-body">
   {#if tab==='Modele i wycena'}
    <div class="section-title"><h3>Pliki do realizacji</h3><span>{selected.models.length} {selected.models.length===1?'model':'modele'} · {pieces(selected)} szt.</span></div>
    {#each selected.models as m}<section class="model"><div class="model-name"><span class="file-icon"><IconCube size={24}/></span><div><strong>{m.name}</strong><small>{m.qty} szt.</small><div class="material-colors"><span class="material-tag">{m.material}</span><span class="color-label"><span class="color-swatch" style:background={swatch(m.color)} aria-hidden="true"></span>{m.color}</span></div></div></div><dl><div><dt>Jakość</dt><dd>{m.quality}</dd></div><div><dt>Wytrzymałość</dt><dd>{m.strength}</dd></div></dl><div class="model-controls"><label>Cena / szt. (zł)<input type="number" min="0" step="0.01" value={m.price} onchange={(e)=>{m.price=Math.max(0,Number(e.currentTarget.value)||0);save();}}/></label><label>Gotowe / {m.qty}<input type="number" min="0" max={m.qty} value={m.done} onchange={(e)=>{m.done=Math.min(m.qty,Math.max(0,Math.floor(Number(e.currentTarget.value)||0)));save();}}/></label><strong>{money(m.price*m.qty)}</strong></div></section>{/each}
    <div class="payment-box"><div><strong>Płatność</strong><small>{selected.paid?'Płatność oznaczona jako otrzymana':'Oczekuje na płatność'}</small></div><button class="secondary" onclick={()=>payment(selected!)}>{selected.paid?'Cofnij oznaczenie':'Oznacz opłacone'}</button></div>
    <section class="delivery"><h3><IconPackage size={18}/>{selected.delivery}</h3><p>{selected.address}</p><small>Adres demonstracyjny</small></section>
   {:else if tab==='Rozliczenie'}
    {#key selected.id}<Finance {orders} {invoices} {selected} mode="Rozliczenie" onopen={openFinance} onsave={save} onpayment={payment}/>{/key}
   {:else if tab==='Pliki'}
    <div class="section-title"><h3>Pliki zamówienia</h3><span>{selected.models.length} pozycji</span></div>
    <p class="helper">Próbka: przypięte pliki są dostępne do odświeżenia strony.</p>
    {#if fileError}<p class="warning" role="alert">{fileError}</p>{/if}
    {#each selected.models as m, i}
     {@const key = selected.id+':'+i}
     {@const file = attachments[key]}
     <section class="order-file">
      <div class="model-name"><span class="file-icon"><IconCube size={24}/></span><div><strong>{m.name}</strong><small>{file ? file.name+' · '+(file.size/1024/1024).toFixed(2)+' MB' : 'Brak pliku źródłowego w próbce'}</small></div></div>
      <div class="material-colors"><span class="material-tag">{m.material}</span><span class="color-label"><span class="color-swatch" style:background={swatch(m.color)} aria-hidden="true"></span>{m.color}</span></div>
      <div class="file-actions"><button class="secondary" disabled={!file} onclick={()=>file&&downloadFile(file)}><IconDownload size={16}/>Pobierz</button><button class="secondary" disabled title="Bezpośrednie otwieranie lokalnych plików w Bambu Studio wymaga integracji z aplikacją."><IconArrowUpRight size={16}/>Bambu Studio</button><label class="file-picker"><input type="file" accept=".stl,.step,.stp,.3mf" aria-label={'Przypnij plik do '+m.name} onchange={(e)=>attach(key,e)}/><span>{file?'Zmień plik':'Przypnij plik'}</span></label></div>
     </section>
    {/each}
   {:else if tab==='Wiadomości'}
    <h3>Rozmowa z klientem</h3><div class="bubble"><strong>{selected.client}</strong><p>{selected.message}</p></div><label class="message-label">Twoja odpowiedź<textarea rows="5" bind:value={selected.draft} placeholder="Napisz odpowiedź dotyczącą realizacji…"></textarea></label><button class="primary" onclick={()=>{save();feedback='Szkic zapisany lokalnie. Wiadomość nie została wysłana.';}}>Zapisz szkic</button><p class="helper">Wysyłanie wiadomości będzie dostępne po podłączeniu panelu do serwera.</p>
   {:else if tab==='Notatki'}
    <h3>Notatki wewnętrzne</h3><p class="helper">Miejsce na ustalenia dotyczące druku. Niewidoczne dla klienta.</p><form onsubmit={(e)=>{e.preventDefault();addNote();}}><label class="message-label">Nowa notatka<textarea rows="4" bind:value={note} placeholder="Np. sprawdzić tolerancję otworu po wydruku" required></textarea></label><button class="primary" disabled={!note.trim()}>Dodaj notatkę</button></form>{#each selected.notes as n}<p class="saved-note">{n}</p>{:else}<p class="helper">Nie dodano jeszcze żadnej notatki.</p>{/each}
   {:else}<h3>Historia realizacji</h3><ol class="history">{#each selected.history as h}<li>{h}</li>{/each}</ol>{/if}
  </div>
  <div class="detail-footer"><p role="status">{feedback}</p><div><span>Wartość modeli<strong>{money(total(selected))}</strong></span><button class="primary" disabled={selected.status==='Zakończone'} onclick={()=>advance(selected!)}>{selected.status==='Nowe'?'Przejdź do wyceny':selected.status==='Zakończone'?'Zakończone':'Dalej: '+stages[stages.indexOf(selected.status)+1]}<IconArrowRight size={17}/></button></div></div>
 </aside>
 {/if}
</div>
