<script lang="ts">
 import { IconArrowUpRight, IconCheck, IconClock, IconSearch, IconFileInvoice, IconArrowRight } from '@tabler/icons-svelte';
 import { total, money, type Order } from './data';
 export type InvoiceDraft = { orderId:string; buyer:string; address:string; nip:string; created:string; lines:{name:string;qty:number;price:number}[] };
 let {orders, invoices, mode, selected=null, onopen, onsave, onpayment}: {orders:Order[];invoices:Record<string,InvoiceDraft>;mode:string;selected?:Order|null;onopen:(o:Order,e?:MouseEvent)=>void;onsave:()=>void;onpayment:(o:Order)=>void}=$props();
 let search=$state('');
 let filter=$state('Wszystkie');
 let message=$state('');
 let shown=$derived(orders.filter(o=>(o.id+' '+o.client).toLowerCase().includes(search.toLowerCase())).filter(o=>filter==='Wszystkie'||mode==='Płatności'&&(filter==='Opłacone'?o.paid:!o.paid)||mode==='Faktury'&&(filter==='Szkice'?!!invoices[o.id]:!invoices[o.id])));
 const sumDraft=(d:InvoiceDraft)=>d.lines.reduce((sum,l)=>sum+l.qty*l.price,0);
 const snapshot=(o:Order)=>o.models.map(m=>({name:m.name,qty:m.qty,price:m.price}));
 function create(o:Order){if(invoices[o.id])return;invoices[o.id]={orderId:o.id,buyer:o.client,address:o.address,nip:'',created:new Date().toISOString(),lines:snapshot(o)};o.history.unshift('Utworzono szkic faktury z pozycji zamówienia.');onsave();message='Szkic zapisany lokalnie.';}
 function refresh(o:Order){invoices[o.id].lines=snapshot(o);o.history.unshift('Zaktualizowano pozycje szkicu faktury.');onsave();message='Pozycje zaktualizowane.';}
</script>
{#if selected}
 <div class="finance-payment"><div><h3>Płatność za zamówienie</h3><p class="helper">Kwota modeli: <strong>{money(total(selected))}</strong></p></div><span class="status" data-status={selected.paid?'complete':'waiting'}>{selected.paid?'Opłacone':'Nieopłacone'}</span></div>
 <button class="secondary" onclick={()=>{onpayment(selected!);message='Zapisano oznaczenie płatności.';}}>{selected.paid?'Cofnij oznaczenie płatności':'Oznacz jako opłacone'}</button>
 <p class="helper">Ręczne oznaczenie w próbce. Brak połączenia z operatorem płatności.</p>
 <section class="invoice-section"><h3>Faktura do {selected.id}</h3>
 {#if invoices[selected.id]}
  {@const draft=invoices[selected.id]}
  <div class="invoice-state"><span class="status" data-status="preparing">Szkic</span><span class="muted">Dokument niewystawiony</span></div>
  <form onsubmit={(e)=>{e.preventDefault();onsave();message='Dane szkicu zapisane.';}}>
   <label class="message-label">Nabywca<input bind:value={draft.buyer} required maxlength="200"/></label>
   <label class="message-label">Adres nabywcy<textarea bind:value={draft.address} required rows="2" maxlength="500"></textarea></label>
   <label class="message-label">NIP (opcjonalnie)<input bind:value={draft.nip} maxlength="20"/></label>
   <div class="invoice-lines">{#each draft.lines as line}<div><span>{line.name}<small>{line.qty} szt. × {money(line.price)}</small></span><strong>{money(line.qty*line.price)}</strong></div>{/each}</div>
   <div class="invoice-total"><span>Suma pozycji szkicu</span><strong>{money(sumDraft(draft))}</strong></div>
   {#if JSON.stringify(draft.lines)!==JSON.stringify(snapshot(selected))}<p class="warning">Pozycje zamówienia zmieniły się od utworzenia szkicu.</p><button type="button" class="secondary" onclick={()=>refresh(selected!)}>Aktualizuj z zamówienia</button>{/if}
   <div class="file-actions"><button class="primary">Zapisz szkic</button><button type="button" class="secondary" disabled title="Wymaga połączenia z systemem fakturowania">Wystaw fakturę</button></div>
  </form>
  <p class="helper">Podatki, numer dokumentu i PDF będą pobierane z systemu fakturowania po jego podłączeniu.</p>
 {:else}
  <div class="invoice-empty"><IconFileInvoice size={32}/><h3>Brak dokumentu</h3><p class="helper">Przygotuj szkic z danych klienta, modeli i ilości z tego zamówienia.</p><button class="primary" onclick={()=>create(selected!)}>Utwórz szkic faktury<IconArrowRight size={16}/></button></div>
 {/if}
 </section><p class="helper" role="status">{message}</p>
{:else}
 <div class="finance-summary">
 {#if mode==='Płatności'}<span>Nieopłacone <strong>{money(orders.filter(o=>!o.paid).reduce((s,o)=>s+total(o),0))}</strong></span><span>Oznaczone jako opłacone <strong>{money(orders.filter(o=>o.paid).reduce((s,o)=>s+total(o),0))}</strong></span>{:else}<span>Do przygotowania <strong>{orders.filter(o=>!invoices[o.id]).length}</strong></span><span>Zapisane szkice <strong>{orders.filter(o=>invoices[o.id]).length}</strong></span>{/if}
 <span class="muted">{mode==='Płatności'?'Rozliczenia demonstracyjne · tylko wartość modeli':'Szkice robocze · bez wystawiania dokumentów'}</span>
 </div>
 <div class="list-tools"><div class="filters" aria-label={'Filtr: '+mode}>{#each (mode==='Płatności'?['Wszystkie','Nieopłacone','Opłacone']:['Wszystkie','Do przygotowania','Szkice']) as f}<button class:chosen={filter===f} aria-pressed={filter===f} onclick={()=>filter=f}>{f}</button>{/each}</div><label class="search"><IconSearch size={18}/><input aria-label={'Szukaj: '+mode} bind:value={search} placeholder="Szukaj zamówienia lub klienta…"/></label></div>
 <div class="table-scroll"><table><thead><tr><th>Zamówienie / klient</th><th>{mode==='Płatności'?'Płatność':'Dokument'}</th><th>{mode==='Płatności'?'Źródło':'Rozliczenie zamówienia'}</th><th class="amount">{mode==='Płatności'?'Kwota modeli':'Suma pozycji'}</th><th>Akcja</th></tr></thead><tbody>
 {#each shown as o}<tr><td><button class="order-link" onclick={(e)=>onopen(o,e)}>{o.id}<IconArrowUpRight size={14}/></button><strong class="client">{o.client}</strong></td><td><span class="status" data-status={mode==='Płatności'?(o.paid?'complete':'waiting'):(invoices[o.id]?'preparing':'quote')}>{mode==='Płatności'?(o.paid?'Opłacone':'Nieopłacone'):(invoices[o.id]?'Szkic':'Brak dokumentu')}</span></td><td>{#if mode==='Płatności'}<span class="muted">Oznaczenie ręczne · demo</span>{:else}<span class="payment" class:paid={o.paid}>{#if o.paid}<IconCheck size={14}/>{:else}<IconClock size={14}/>{/if}{o.paid?'Opłacone':'Nieopłacone'}</span>{/if}</td><td class="amount">{money(mode==='Faktury'&&invoices[o.id]?sumDraft(invoices[o.id]):total(o))}</td><td><span class="finance-next">{mode==='Płatności'?'Szczegóły rozliczenia':invoices[o.id]?'Otwórz szkic':'Przygotuj szkic'}<IconArrowRight size={15}/></span></td></tr>{/each}
 </tbody></table>{#if !shown.length}<div class="empty"><h2>Brak pasujących pozycji</h2><p>Zmień wyszukiwanie lub filtr.</p><button class="secondary" onclick={()=>{search='';filter='Wszystkie';}}>Wyczyść filtry</button></div>{/if}</div><footer class="list-footer"><span>{shown.length} z {orders.length} zamówień</span><span>Powiązanie przez numer zamówienia</span></footer>
{/if}
