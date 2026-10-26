<script lang="ts">
  import { IconArrowLeft, IconArrowRight, IconCheck, IconPackage, IconMapPin } from '@tabler/icons-svelte';
  import Button from './ui/Button.svelte';
  import { onMount, untrack } from 'svelte';
  import Checkout from '../../pages/checkout/Checkout.svelte';
  import type { Contact, DeliveryMethod } from '../../lib/types';
  import * as orderApi from '../../lib/api';
  import { prepareCheckout } from '../lib/api';
  import type { Material, Model, Quote } from '../lib/types';
  import { settingsSummary } from '../lib/modelSettings';
  let { draft, onback }: {draft:{models:Model[];materials:Material[];quote:Quote;delivery:string};onback:()=>void}=$props();
  let form = $state<HTMLFormElement>();
  let reviewed=$state(false);
  let invoice=$state(false);
  let checkoutStage=$state(2);
  let delivery=$state(untrack(()=>draft.delivery));
  let preparing=$state(false),error=$state('');
  let contactKey='',edited=false;
  let data=$state({name:'',email:'',phone:'',company:'',taxId:'',street:'',postal:'',city:'',country:'Polska',notes:''});
  const manual=$derived(draft.quote.manual || !!data.notes.trim());
  const money=(value:number)=>new Intl.NumberFormat('pl-PL',{style:'currency',currency:'PLN'}).format(value);
  const contact=$derived<Contact>({name:data.name,email:data.email,phone:data.phone,street:data.street,postalCode:data.postal,city:data.city,company:invoice?data.company:'',nip:invoice?data.taxId:'',pickupPoint:''});
  onMount(()=>{
    let active=true;
    void(async()=>{
      try {
        const user=await orderApi.getSession();
        if(!active)return;
        contactKey='rapidmaker.checkout-contact.v2.'+(user?.id ?? 'guest');
        const profile=user?await orderApi.getProfile():null;
        if(!active)return;
        if(!edited && profile)applyContact(profile.contact,!!profile.contact.company);
        const saved=JSON.parse(sessionStorage.getItem(contactKey) ?? 'null');
        if(!edited && saved?.contact){applyContact(saved.contact,!!saved.invoice);data.notes=typeof saved.notes==='string'?saved.notes:'';}
        else if(!edited && user)data.email=user.email;
      }catch{ if(active)error='Nie udało się odczytać zapisanych danych. Możesz wpisać je ponownie.'; }
    })();
    return()=>{active=false;};
  });
  function applyContact(value:Partial<Contact>,withInvoice:boolean){
    data={...data,name:value.name??'',email:value.email??'',phone:value.phone??'',street:value.street??'',postal:value.postalCode??'',city:value.city??'',company:value.company??'',taxId:value.nip??''};
    invoice=withInvoice;
  }
  function save(){
    (form?.elements.namedItem('phone') as HTMLInputElement | null)?.setCustomValidity('');
    edited=true;
    if(contactKey)try{sessionStorage.setItem(contactKey,JSON.stringify({contact,invoice,notes:data.notes}));}catch{/* Optional draft storage. */}
  }
  function editContact(value:Contact,withInvoice:boolean,method:DeliveryMethod){
    applyContact(value,withInvoice);
    delivery=method==='pickup'?'pickup':'shipping';
    reviewed=false;
    requestAnimationFrame(()=>form?.querySelector('input')?.focus());
  }
  function updateStage(stage:number){checkoutStage=stage;}
  async function review(event:SubmitEvent){
    event.preventDefault();
    if(preparing || !form)return;
    const phone=form.elements.namedItem('phone') as HTMLInputElement;
    phone.setCustomValidity(/^\+?\d{9,15}$/.test(data.phone.replace(/[\s()-]/g,''))?'':'Podaj 9 cyfr lub numer z prefiksem kraju.');
    if(!form.reportValidity())return;
    if(draft.models.some(model=>model.sample || !model.analysis)){error='Przykłady pokazują wygląd konfiguratora. Aby zamówić wydruk, wróć i dodaj własne pliki.';return;}
    preparing=true;error='';
    try{
      await prepareCheckout(draft.models,delivery,draft.quote.manual,data.notes);
      save();reviewed=true;
      requestAnimationFrame(()=>{document.getElementById('order-title')?.focus();window.scrollTo({top:0,behavior:'instant'});});
    }catch(cause){error=cause instanceof Error?cause.message:'Nie udało się przygotować dostawy i płatności.';}
    finally{preparing=false;}
  }

</script>

<main class="order-page shell" id="zamowienie">
  <button class="order-back" onclick={onback}><IconArrowLeft size={17}/> Wróć do konfiguracji</button>
  <div class="order-heading"><div><h1 id="order-title" tabindex="-1">Nadajemy kształt.<br/><span>Teraz szczegóły.</span></h1><p>Twój projekt jest gotowy na kolejny krok. Podaj dane do realizacji.</p></div><div class="order-stage"><span><IconCheck size={15}/> Konfiguracja</span><strong aria-current={reviewed?undefined:'step'}>{#if reviewed}<IconCheck size={15}/>{/if} Dane zamówienia</strong><span class:stage-current={reviewed} aria-current={reviewed?'step':undefined}>{checkoutStage===4?'Płatność':checkoutStage===3?'Podsumowanie':'Dostawa i płatność'}</span></div></div>
  {#if error}<p class="order-error" role="alert">{error}</p>{/if}
  <div class="order-layout" class:checkout-active={reviewed}>
    <div class="order-form-area">
      {#if reviewed}
        <Checkout embedded initialContact={contact} initialInvoice={invoice} onEditContact={editContact} onStage={updateStage} />
      {:else}
        <form bind:this={form} onsubmit={review} oninput={save} onchange={save}>
          <section class="order-form-section" aria-labelledby="order-contact-heading">
            <h2 id="order-contact-heading">Z kim tworzymy?</h2><p>Dane kontaktowe do omówienia projektu.</p>
            <div class="order-fields">
              <label class="order-field full">Imię i nazwisko<input class="input" bind:value={data.name} name="name" autocomplete="name" required maxlength="120" pattern=".*\S.*" placeholder="Jan Kowalski"/></label>
              <label class="order-field">E-mail<input class="input" bind:value={data.email} name="email" type="email" autocomplete="email" required maxlength="254" placeholder="jan@przyklad.pl"/></label>
              <label class="order-field">Telefon<input class="input" bind:value={data.phone} name="phone" type="tel" autocomplete="tel" required pattern={'[+0-9 .\\(\\)\\-]{7,25}'} title="Podaj numer telefonu, np. +48 123 456 789" placeholder="+48 123 456 789"/></label>
            </div>
          </section>
          <section class="order-form-section" aria-label="Faktura i uwagi">
            <label class="check-label"><input type="checkbox" bind:checked={invoice}/> Potrzebuję faktury na firmę</label>
            {#if invoice}<div class="order-fields"><label class="order-field">Nazwa firmy<input class="input" bind:value={data.company} name="company" autocomplete="organization" required pattern=".*\S.*" maxlength="180"/></label><label class="order-field">NIP<input class="input" bind:value={data.taxId} name="taxId" required pattern="[0-9]{10}" maxlength="10" inputmode="numeric" placeholder="Numer identyfikacji podatkowej"/></label></div>{/if}
            <label class="order-field order-notes-field">Co jeszcze powinniśmy wiedzieć? <span>Opcjonalnie</span><textarea class="input" bind:value={data.notes} name="notes" rows="4" maxlength="2000" placeholder="Zastosowanie części, ważne wymiary, oczekiwany termin…"></textarea></label>
          </section>
          <section class="order-form-section" aria-labelledby="order-delivery-heading">
            <div class="order-section-title"><h2 id="order-delivery-heading">{delivery==='shipping'?'Dokąd wysyłamy?':'Odbierasz w pracowni.'}</h2>{#if delivery==='shipping'}<IconPackage size={23}/>{:else}<IconMapPin size={23}/>{/if}</div>
            {#if delivery==='shipping'}
              <p>Wybór przewoźnika i pełny koszt dostawy zobaczysz przed płatnością.</p>
              <p class="order-address-note">Adres wpiszesz w następnym kroku z podpowiedziami. Tam wybierzesz kuriera albo Paczkomat — bez ponownego wpisywania danych kontaktowych.</p>
            {:else}<p>Zielony Krąg 1, 05-500 Piaseczno.<br/>Odezwiemy się, żeby ustalić termin odbioru.</p>{/if}
          </section>
          <div class="order-submit"><Button type="submit" disabled={preparing}>{preparing?'Przygotowuję…':manual?'Przejdź do indywidualnej wyceny':'Wybierz dostawę i zapłać'}<IconArrowRight size={18}/></Button><p>Zamówienie opłacisz dopiero po sprawdzeniu pełnej kwoty. Indywidualne wymagania najpierw potwierdzi zespół. <a href="/prywatnosc" target="_blank" rel="noopener">Zasady prywatności</a>.</p></div>
        </form>
      {/if}
    </div>
    {#if !reviewed}<aside class="order-summary" aria-label="Podsumowanie modeli">
      <div class="order-summary-title"><h2>Twój projekt.</h2><span>{draft.models.length} {draft.models.length===1?'model':'modeli'}</span></div>
      <ul>{#each draft.models as model}<li><div><strong>{model.name}</strong><span>{model.quantity} szt.</span></div><p>{settingsSummary(model.settings!,draft.materials)}</p>{#if !manual || draft.quote.provisional}<span class="order-line-price">{money(draft.quote.lines.find(line=>line.id===model.id)?.subtotal??0)}</span>{/if}</li>{/each}</ul>
      <div class="order-summary-delivery"><span>{delivery==='shipping'?'Wysyłka':'Odbiór osobisty'}</span><strong>{delivery==='shipping'?'W następnym kroku':money(0)}</strong></div>
      <div class="order-total"><span>{draft.quote.provisional?'Szacowana cena wydruku':manual?'Wycena indywidualna':'Wydruki i przygotowanie'}</span><strong>{manual && !draft.quote.provisional?'Ustalimy razem':money(draft.quote.total)}</strong></div>
      {#if draft.quote.provisional}<p class="order-price-note">{draft.quote.reason} Ostateczną ofertę otrzymasz przed płatnością.</p>{/if}
      <p class="order-price-note">Produkcja do 14 dni kalendarzowych, chyba że oferta określa inny termin. Dostawa i opakowanie zostaną wyliczone przed płatnością.</p>
      <button class="order-back order-edit" onclick={onback}>Zmień modele lub ustawienia<IconArrowLeft size={16}/></button>
    </aside>{/if}
  </div>
</main>

<style>
  .order-layout.checkout-active{display:block}.order-error{padding:16px;border:1px solid var(--danger);color:var(--danger);margin-bottom:24px;}

  .order-page{padding-block:32px 80px}.order-back{display:inline-flex;align-items:center;gap:9px;min-height:44px;padding:0;background:transparent;border:0;color:var(--muted);font-size:13px;text-align:left;transition:color 140ms ease}.order-back:active{transform:scale(.98)}
  .order-heading{display:flex;justify-content:space-between;align-items:end;gap:32px;margin:24px 0 44px}.order-heading h1{font-size:clamp(38px,4.5vw,64px);max-width:none;line-height:1.06}.order-heading h1 span{color:var(--accent)}.order-heading p{margin-top:18px;max-width:49ch;color:var(--muted)}
  .order-stage{display:flex;flex-wrap:wrap;gap:12px 20px;font-size:12px;padding-bottom:4px}.order-stage span:first-child{display:flex;align-items:center;gap:6px;color:var(--accent)}.order-stage span:last-child{color:var(--muted)}.order-stage strong{display:flex;align-items:center;gap:6px}.order-stage .stage-current{color:var(--ink);font-weight:700}
  .order-layout{display:grid;grid-template-columns:minmax(0,1.25fr) minmax(0,1fr);gap:72px;align-items:start}.order-form-section{padding-block:28px;border-top:1px solid var(--line)}.order-form-section h2{font-size:24px}.order-form-section>p{font-size:14px;color:var(--muted);margin-top:8px}.order-fields{display:grid;grid-template-columns:1fr 1fr;gap:20px;margin-top:24px}.order-field{display:flex;flex-direction:column;gap:9px;min-width:0;font-size:13px;font-weight:600}.order-field.full{grid-column:1/-1}.order-field .input{width:100%;min-height:50px;border:1px solid var(--control-line);background:var(--surface);color:var(--ink);padding:12px 14px;font-size:16px;font-weight:400;border-radius:var(--control-radius)}.order-field .input::placeholder{color:var(--muted);opacity:1}.order-section-title{display:flex;align-items:center;justify-content:space-between;gap:16px}.order-section-title :global(svg){color:var(--accent)}.order-notes-field{margin-top:26px}.order-field>span{font-size:12px;color:var(--muted);font-weight:400}.order-field textarea{resize:vertical}.order-submit{border-top:1px solid var(--line);padding-top:24px}.order-submit :global(.button){width:100%;justify-content:space-between;min-height:52px}.order-submit p{font-size:12px;color:var(--muted);margin-top:14px}
  .order-summary{position:sticky;top:28px;border-top:2px solid var(--accent);background:var(--surface);padding:28px}.order-summary-title{display:flex;align-items:center;justify-content:space-between;gap:16px}.order-summary-title h2{font-size:27px}.order-summary-title>span{color:var(--muted);font-size:12px}.order-summary ul{list-style:none;padding:0;margin:24px 0 0;max-height:420px;overflow:auto;scrollbar-gutter:stable}.order-summary li{padding:18px 0;border-top:1px solid var(--line)}.order-summary li>div{display:flex;align-items:start;justify-content:space-between;gap:16px;font-size:13px}.order-summary li strong{overflow-wrap:anywhere}.order-summary li>div span{white-space:nowrap}.order-summary li p{font-size:12px;color:var(--muted);margin-top:8px}.order-line-price{display:block;text-align:right;font-size:13px;margin-top:10px}.order-summary-delivery{display:flex;justify-content:space-between;gap:16px;font-size:13px;padding-block:20px;border-top:1px solid var(--line)}.order-total{border-top:1px solid var(--line);padding-top:22px;display:flex;flex-direction:column;gap:8px}.order-total>span{color:var(--muted);font-size:13px}.order-total strong{font-family:var(--display);font-size:34px;line-height:1.2;color:var(--accent)}.order-price-note{font-size:12px;color:var(--muted);margin-top:14px}.order-edit{margin-top:18px;color:var(--ink);gap:12px}
  @media(hover:hover) and (pointer:fine){.order-back:hover{color:var(--accent)}}
  @media(max-width:1000px){.order-layout{gap:32px}.order-heading{display:block}.order-stage{margin-top:24px}.order-summary{padding:22px}}
  @media(max-width:760px){.order-page{padding-block:20px 48px}.order-heading{margin:20px 0 30px}.order-layout{grid-template-columns:1fr;gap:32px}.order-heading h1{font-size:clamp(34px,8vw,52px)}.order-fields{grid-template-columns:1fr;gap:18px}.order-field.full{grid-column:auto}.order-summary{position:static}.order-stage{gap:10px 16px}.order-summary ul{max-height:none}}
</style>
