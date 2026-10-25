<script lang="ts">
  import LegalNotice from './LegalNotice.svelte';
  import { onMount } from 'svelte';
  import {
    publishManualOffer,
    manualPayment,
    validOfferAmount,
    type ManualOffer,
    type ManualPayment as Payment,
  } from '../lib/manualQuotes';
  import { hundredths } from '../lib/operator';
  import { money } from '../lib/format';
  import ManualPayment from './ManualPayment.svelte';
  let {
    orderId,
    offer,
    adminMode = false,
    onchange,
  }: {
    orderId: string;
    offer: ManualOffer;
    adminMode?: boolean;
    onchange: () => void | Promise<void>;
  } = $props();
  let total = $state(''),
    shipping = $state(''),
    scope = $state('');
  let error = $state(''),
    notice = $state(''),
    busy = $state(false);
  let payment = $state<Payment | null>(null);
  let publication = $state<{
    version: number;
    amount_grosze: number;
    shipping_grosze: number;
    scope: string;
  } | null>(null);
  const requestId = crypto.randomUUID();
  let currentTime = $state(Date.now()),
    active = true,
    timer: ReturnType<typeof setTimeout>;
  let editVersion = -1;
  const amount = $derived.by(() => {
    try {
      return validOfferAmount(offer);
    } catch {
      return null;
    }
  });
  const expired = $derived(
    offer.state === 'expired' ||
      (offer.state === 'offered' &&
        offer.valid_until !== null &&
        offer.valid_until * 1000 <= currentTime),
  );
  const editable = $derived(
    adminMode &&
      ['awaiting_quote', 'offered'].includes(offer.state) &&
      !offer.payment_started &&
      !expired &&
      !busy &&
      !publication,
  );
  $effect(() => {
    if (offer.version !== editVersion) {
      editVersion = offer.version;
      if (publication && offer.version > publication.version) {
        notice =
          offer.amount_grosze === publication.amount_grosze &&
          offer.shipping_grosze === publication.shipping_grosze &&
          offer.scope === publication.scope
            ? 'Oferta zapisana i przekazana do wysyłki email.'
            : 'Oferta zmieniła się na innym urządzeniu. Sprawdź aktualne dane.';
        publication = null;
      }
      total =
        offer.amount_grosze === null
          ? ''
          : (offer.amount_grosze / 100).toFixed(2).replace('.', ',');
      shipping =
        offer.shipping_grosze === null
          ? '0,00'
          : (offer.shipping_grosze / 100).toFixed(2).replace('.', ',');
      scope = offer.scope ?? '';
    }
  });
  onMount(() => {
    const clock = setInterval(() => {
      currentTime = Date.now();
    }, 30_000);
    return () => {
      active = false;
      clearInterval(clock);
      clearTimeout(timer);
    };
  });
  async function publish() {
    if (busy || !adminMode) return;
    try {
      if (!publication) {
        const amount_grosze = hundredths(total, 'Cena brutto', 99_999_999),
          shipping_grosze = hundredths(shipping, 'Dostawa', 99_999_999);
        if (
          !amount_grosze ||
          shipping_grosze > amount_grosze ||
          !scope.trim() ||
          new TextEncoder().encode(scope).length > 4000
        )
          throw new Error(
            'Podaj dodatnią cenę całkowitą, dostawę nie większą od ceny i opis zakresu.',
          );
        if (
          !confirm(
            `Opublikować ofertę ${money(amount_grosze / 100)} brutto, w tym dostawa ${money(shipping_grosze / 100)}? Klient otrzyma powiadomienie email.`,
          )
        )
          return;
        publication = {
          version: offer.version,
          amount_grosze,
          shipping_grosze,
          scope: scope.trim(),
        };
      }
      busy = true;
      error = '';
      await publishManualOffer(orderId, publication);
      if (active) {
        notice = 'Oferta zapisana i przekazana do wysyłki email.';
        publication = null;
        await onchange();
      }
    } catch (cause) {
      if (active) {
        error = cause instanceof Error ? cause.message : 'Nie udało się zapisać oferty.';
        await onchange();
      }
    } finally {
      if (active) busy = false;
    }
  }
  async function pay() {
    if (busy || adminMode || amount === null) return;
    clearTimeout(timer);
    busy = true;
    error = '';
    notice = '';
    let attempts = 0;
    async function poll() {
      try {
        const result = await manualPayment(orderId, offer.version, requestId);
        if (!active) return;
        if (result.order_id !== orderId || result.total_grosze !== amount)
          throw new Error('Dane płatności różnią się od aktualnej oferty. Odśwież zamówienie.');
        payment = result;
        if (['creating', 'unknown', 'processing', 'manual_review'].includes(result.state)) {
          notice =
            result.state === 'manual_review'
              ? 'Płatność wymaga sprawdzenia przez obsługę. Nie rozpoczynaj kolejnej.'
              : 'Sprawdzamy tę samą płatność. Nie składaj kolejnego zlecenia.';
          if (++attempts < 24) timer = setTimeout(poll, 5000);
          else {
            busy = false;
            notice = 'Potwierdzenie zajmuje dłużej. Sprawdź ponownie status tej samej płatności.';
          }
        } else {
          busy = false;
          if (result.state === 'paid') {
            notice = 'Serwer potwierdził płatność.';
            await onchange();
          }
        }
      } catch (cause) {
        if (active) {
          error =
            cause instanceof Error
              ? cause.message
              : 'Nie znamy wyniku płatności. Sprawdź ponownie.';
          busy = false;
        }
      }
    }
    await poll();
  }
</script>

<section class="manual-offer" aria-label="Wycena indywidualna">
  <header>
    <h3>{offer.state === 'awaiting_quote' ? 'Oczekuje na wycenę' : 'Oferta indywidualna'}</h3>
    <button class="refresh" disabled={busy} onclick={onchange}>Odśwież</button>
  </header>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if notice}<p class="muted" role="status">{notice}</p>{/if}
  {#if amount === null && offer.state !== 'awaiting_quote' && !expired}<p
      class="error"
      role="alert"
    >
      Nie udało się odczytać kwoty oferty. Odśwież zamówienie.
    </p>{/if}
  {#if amount !== null}<p class="offer-amount">
      {money(amount / 100)}
      <small>brutto · dostawa w cenie: {money((offer.shipping_grosze ?? 0) / 100)}</small>
    </p>{:else}<p class="muted">
      Zespół sprawdzi pliki, zakres wykonania i dostawę. Cenę otrzymasz tutaj oraz w powiadomieniu
      email.
    </p>{/if}
  {#if offer.notes}<p class="scope muted">Wymagania klienta: {offer.notes}</p>{/if}
  {#if offer.scope}<p class="scope">{offer.scope}</p>{/if}
  {#if offer.valid_until && offer.state === 'offered'}<p class="muted">
      {expired ? 'Oferta wygasła' : 'Oferta ważna do'}
      {new Date(offer.valid_until * 1000).toLocaleString('pl-PL')}.
    </p>{/if}
  {#if adminMode}
    {#if ['awaiting_quote', 'offered'].includes(offer.state) && !offer.payment_started && !expired}
      <form
        onsubmit={(event) => {
          event.preventDefault();
          void publish();
        }}
      >
        <div class="amount-fields">
          <label
            >Całość brutto (zł)<input
              inputmode="decimal"
              autocomplete="off"
              bind:value={total}
              disabled={!editable}
              required
            /></label
          ><label
            >W tym dostawa (zł)<input
              inputmode="decimal"
              autocomplete="off"
              bind:value={shipping}
              disabled={!editable}
              required
            /></label
          >
        </div>
        <label
          >Zakres wykonania i dostawy<textarea
            rows="3"
            maxlength="4000"
            bind:value={scope}
            disabled={!editable}
            required
            placeholder="Co wykonamy, termin, metoda i zakres dostawy…"></textarea></label
        >
        <small
          >Oferta jest ważna 7 dni. Cena całkowita zawiera dostawę. Po rozpoczęciu płatności nie
          można jej zmienić.</small
        >
        <div>
          <button class="action" disabled={busy}
            >{publication
              ? 'Sprawdź / ponów zapis oferty'
              : 'Opublikuj ofertę i powiadom klienta'}</button
          >
        </div>
      </form>
    {:else}<p class="muted">
        {expired
          ? 'Oferta wygasła. Potrzebne jest nowe zgłoszenie z dostępnymi plikami.'
          : 'Rozpoczęto już płatność. Oferta jest zablokowana przed zmianami.'}
      </p>{/if}
  {:else if offer.state !== 'paid'}
    {#if payment?.state === 'failed' || payment?.state === 'expired' || expired}<p class="error">
        Ta oferta lub próba płatności wygasła albo została zakończona. Przygotowanie kolejnej wymaga
        nowego zgłoszenia.
      </p>
      <a href="/wycena">Przejdź do konfiguratora</a>
    {:else if payment?.state === 'open' && payment.client_secret}{#key payment.attempt_id}<ManualPayment
          {payment}
          {orderId}
          {onchange}
        />{/key}
    {:else if amount !== null && (!expired || offer.state === 'payment_started')}<LegalNotice
        preparing
      /><button class="action" disabled={busy} onclick={pay}
        >{busy
          ? 'Sprawdzamy płatność…'
          : offer.state === 'payment_started' || payment
            ? 'Sprawdź / kontynuuj tę płatność'
            : 'Przygotuj płatność za ofertę'}</button
      >{/if}
  {/if}
</section>

<style>
  .manual-offer,
  form {
    display: grid;
    gap: calc(var(--space, 8px) * 2);
    min-width: 0;
  }
  .manual-offer {
    margin: calc(var(--space, 8px) * 3) 0;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: calc(var(--space, 8px) * 2);
  }
  h3,
  p {
    margin: 0;
  }
  h3 {
    font-size: 17px;
  }
  .muted,
  small {
    color: var(--muted);
    font-size: 12px;
    line-height: 1.6;
  }
  .offer-amount {
    font-size: 25px;
    font-variant-numeric: tabular-nums;
  }
  .offer-amount small {
    display: block;
  }
  .scope {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    line-height: 1.6;
  }
  .amount-fields {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: calc(var(--space, 8px) * 2);
  }
  label {
    display: grid;
    gap: var(--space, 8px);
    font-size: 12px;
  }
  input,
  textarea {
    width: 100%;
    min-width: 0;
    padding: calc(var(--space, 8px) * 1.5);
    background: var(--soft);
    color: var(--ink);
    border: 1px solid var(--control-line);
    border-radius: var(--control-radius, var(--radius-sm, 3px));
    font: inherit;
  }
  .action {
    padding: calc(var(--space, 8px) * 1.5) calc(var(--space, 8px) * 2);
    border: 0;
    border-radius: var(--control-radius, var(--radius-sm, 3px));
    color: var(--on-accent);
    background: var(--accent);
    cursor: pointer;
    font: inherit;
    font-size: 12px;
    font-weight: 700;
  }
  .refresh {
    border: 0;
    background: transparent;
    color: var(--accent);
    cursor: pointer;
    font: inherit;
    font-size: 12px;
  }
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 3px;
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .error {
    color: var(--danger);
    font-size: 12px;
  }
  @media (max-width: 500px) {
    .amount-fields {
      grid-template-columns: 1fr;
    }
  }
</style>
