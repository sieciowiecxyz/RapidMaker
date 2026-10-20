<script lang="ts">
  import { tick } from 'svelte';
  import {
    getOrder,
    updateOrder,
    sendOffer,
    refundOrder,
    createShippingLabel,
    retryPayment,
    refreshOrderPayment,
    openOrderPayment,
    reorder,
  } from '../lib/api';
  import {
    money,
    date,
    orderStatus,
    paymentStatus,
    printConfiguration,
    parcels,
  } from '../lib/format';
  import type { Order, OrderStatus as Status, PaymentScenario } from '../lib/types';
  import { navigate } from '../lib/router';
  import Button from './ui/Button.svelte';
  import OrderStatus from './OrderStatus.svelte';
  import OrderChat from './OrderChat.svelte';
  import ParcelPlan from './ParcelPlan.svelte';
  let { id, adminMode = false }: { id: string; adminMode?: boolean } = $props();
  let order = $state<Order | null>(null);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state('');
  let success = $state('');
  let status = $state<Status>('preparing');
  let leadTime = $state('');
  let notes = $state('');
  let offer = $state(0);
  let reason = $state('');
  let scenario = $state<PaymentScenario>('success');
  let showDocument = $state(false);
  function assign(value: Order) {
    order = value;
    status = value.status;
    leadTime = value.leadTime;
    notes = value.notes;
    offer = value.quote.total;
  }
  async function load() {
    loading = true;
    error = '';
    try {
      assign(await getOrder(id));
    } catch (e) {
      error = (e as Error).message;
    } finally {
      loading = false;
    }
    await tick();
    if (order && ['#rozmowa', '#platnosc'].includes(location.hash)) {
      document
        .getElementById(location.hash.slice(1))
        ?.scrollIntoView({ behavior: 'instant', block: 'start' });
    }
  }
  $effect(() => {
    id;
    void load();
  });
  async function act(action: () => Promise<Order>, message: string) {
    busy = true;
    error = '';
    success = '';
    try {
      assign(await action());
      success = message;
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
  async function again() {
    busy = true;
    error = '';
    try {
      await reorder(id);
      navigate('/wycena');
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
  async function pay() {
    busy = true;
    error = '';
    success = '';
    try {
      const payment = await openOrderPayment(id);
      window.location.assign(payment.url);
    } catch (e) {
      error = (e as Error).message;
      busy = false;
    }
  }
  const canOffer = $derived(
    order &&
      !order.sandboxPayment &&
      !['paid', 'refunded'].includes(order.payment) &&
      order.status !== 'cancelled',
  );
  const steps = ['Płatność', 'Przygotowanie', 'Druk', 'Wysyłka / odbiór'];
  let step = $derived(
    !order || order.payment !== 'paid'
      ? 0
      : order.status === 'preparing'
        ? 1
        : order.status === 'printing'
          ? 2
          : ['shipped', 'ready', 'completed'].includes(order.status)
            ? 3
            : 0,
  );
</script>

<div class="page stack">
  <a
    class="back"
    href={adminMode ? '/admin/zamowienia' : '/konto/zamowienia'}
    onclick={(event) => {
      event.preventDefault();
      navigate(adminMode ? '/admin/zamowienia' : '/konto/zamowienia');
    }}>← Wszystkie zamówienia</a
  >
  {#if loading}<div class="panel" role="status">Ładowanie szczegółów…</div>{:else if !order}<div
      class="panel stack"
    >
      <h1>Nie można otworzyć zamówienia</h1>
      <p class="alert error" role="alert">{error}</p>
      <Button onclick={load}>Spróbuj ponownie</Button>
    </div>{:else}
    <div class="page-heading">
      <div>
        <p class="eyebrow">{adminMode ? 'Szczegóły zlecenia' : 'Twój projekt'}</p>
        <h1>{order.number}</h1>
        <p class="muted">Złożone {date(order.createdAt)} · {order.contact.name}</p>
      </div>
      <OrderStatus status={order.status} />
    </div>
    {#if error}<p class="alert error" role="alert">{error}</p>{/if}{#if success}<p
        class="alert success"
        role="status"
      >
        {success}
      </p>{/if}
    <ol class="timeline" aria-label="Postęp realizacji">
      {#each steps as label, index}<li class:active={index <= step}>
          <span>{index + 1}</span>{label}
        </li>{/each}
    </ol>
    <div class="grid-2">
      <section class="panel stack">
        <h2>Modele i ustawienia</h2>
        {#each order.models as model (model.id)}<article class="model">
            <div class="preview" aria-label="Miejsce na podgląd 3D">
              ◇<small>Podgląd 3D wkrótce</small>
            </div>
            <div>
              <h3>{model.name}</h3>
              <p class="muted">{model.dimensions} · {model.parts} część · {model.weight}</p>
              <p>{model.options.materialId.toUpperCase()} · {model.options.color}</p>
              <p class="muted">{model.options.quantity} szt.</p>
              <details class="configuration">
                <summary>Parametry wydruku</summary>
                <ul class="print-settings">
                  {#each printConfiguration(model) as setting}<li>{setting}</li>{/each}
                </ul>
              </details>
              {#if model.notes}<p>Uwagi: {model.notes}</p>{/if}
            </div>
          </article>{/each}{#if !adminMode}<Button
            variant="secondary"
            disabled={busy}
            onclick={again}>Zamów ponownie ↗</Button
          ><small class="muted">Przed zakupem ponownie sprawdzimy dostępność i aktualną cenę.</small
          >{/if}
      </section>
      <section id="platnosc" class="panel stack">
        <h2>Płatność i dostawa</h2>
        <dl>
          <div>
            <dt>Wydruki</dt>
            <dd>{money(order.quote.subtotal)}</dd>
          </div>
          {#if order.quote.checkout?.packaging_grosze}
            <div>
              <dt>Opakowania</dt>
              <dd>{money(order.quote.checkout.packaging_grosze / 100)}</dd>
            </div>
          {/if}
          <div>
            <dt>Dostawa · {parcels(order.quote.packages)}</dt>
            <dd>{money(order.quote.delivery)}</dd>
          </div>
          <div class="total">
            <dt>{order.quote.manual ? 'Szacunek do potwierdzenia' : 'Razem'}</dt>
            <dd>{money(order.quote.total)}</dd>
          </div>
        </dl>
        <p class="badge">{paymentStatus[order.payment]}</p>
        {#if order.sandboxPayment}
          <div class="payment-actions">
            {#if !adminMode && !['paid', 'refunded'].includes(order.payment) && order.status !== 'cancelled'}
              <Button disabled={busy} onclick={pay}>Przejdź do płatności ↗</Button>
            {/if}
            <Button
              variant="secondary"
              disabled={busy}
              onclick={() =>
                act(() => refreshOrderPayment(id), 'Sprawdzono status płatności w Stripe.')}
              >Sprawdź płatność</Button
            >
          </div>
          <small class="muted">Stripe — płatność testowa. Status jest sprawdzany u operatora.</small
          >
        {/if}
        <p class="muted">Termin: {order.leadTime}</p>
        {#if order.sandboxPayment}<p>
            Dostawa: <strong>{order.sandboxPayment.offer.name}</strong>
          </p>{/if}
        <p>
          {order.delivery === 'pickup'
            ? 'Odbiór osobisty — po potwierdzeniu gotowości skontaktujemy się w sprawie odbioru.'
            : order.delivery === 'locker'
              ? `Paczkomat InPost · ${order.contact.pickupPoint}`
              : `${order.contact.street}, ${order.contact.postalCode} ${order.contact.city}`}
        </p>
        {#if order.tracking}<p>Numer śledzenia: <strong>{order.tracking}</strong></p>
          <small class="muted">Numer demonstracyjny — bez połączenia z przewoźnikiem.</small
          >{/if}{#if order.label}<p class="badge">{order.label}</p>{/if}
        {#if order.document}<Button
            variant="secondary"
            onclick={() => (showDocument = !showDocument)}>Podgląd dokumentu sprzedaży</Button
          >{#if showDocument}<div class="alert">
              <strong>{order.document}</strong>
              <p>
                Kwota: {money(order.quote.total)}. To przykładowy dokument do makiety, bez znaczenia
                księgowego.
              </p>
            </div>{/if}{/if}
        {#if order.quote.checkout}
          <ParcelPlan
            packing={order.quote.checkout.packing}
            models={order.models}
            delivery={order.delivery}
          />
        {/if}
        {#if !order.sandboxPayment && !adminMode && !order.quote.manual && !['paid', 'refunded'].includes(order.payment) && order.status !== 'cancelled'}<label
            for="payment-scenario">Scenariusz płatności demo</label
          ><select id="payment-scenario" bind:value={scenario}
            ><option value="success">Sukces</option><option value="pending">Oczekiwanie</option
            ><option value="cancelled">Anulowanie</option><option value="error">Błąd</option
            ></select
          ><Button
            disabled={busy}
            onclick={() =>
              act(() => retryPayment(id, scenario), 'Zaktualizowano płatność demonstracyjną.')}
            >Opłać / ponów płatność</Button
          >{/if}
        {#if order.quote.manual}<p class="alert">
            Zlecenie wymaga ręcznej wyceny. Ustalimy cenę oraz sposób dostawy w rozmowie.
          </p>{/if}{#if order.offerReason}<p class="alert">
            Oferta obsługi: {order.offerReason}
          </p>{/if}
      </section>
    </div>
    {#if adminMode}<div class="grid-2">
        <section class="panel stack">
          <h2>Produkcja</h2>
          <label for="order-status">Status</label><select id="order-status" bind:value={status}
            >{#each Object.entries(orderStatus) as [value, label]}<option {value}>{label}</option
              >{/each}</select
          ><label for="lead-time">Termin realizacji</label><input
            id="lead-time"
            bind:value={leadTime}
          /><label for="private-notes">Prywatne notatki zespołu</label><textarea
            id="private-notes"
            bind:value={notes}
            rows="3"></textarea><Button
            disabled={busy}
            onclick={() =>
              act(
                () => updateOrder(id, { status, leadTime, notes }),
                'Zapisano ustalenia produkcji.',
              )}>Zapisz zmiany</Button
          ><small class="muted">Notatki widzi tylko administrator demo.</small>
        </section>
        <section class="panel stack">
          <h2>{canOffer ? 'Oferta, płatność i wysyłka' : 'Płatność i kontakt'}</h2>
          {#if canOffer}<label for="offer-price">Łączna cena oferty (zł)</label><input
              id="offer-price"
              type="number"
              min="0.01"
              step="0.01"
              bind:value={offer}
            /><label for="offer-reason">Powód zmiany ceny</label><textarea
              id="offer-reason"
              bind:value={reason}
              rows="2"
              placeholder="Np. dodatkowe przygotowanie powierzchni"></textarea><Button
              variant="secondary"
              disabled={busy ||
                !reason.trim() ||
                ['paid', 'refunded'].includes(order.payment) ||
                order.status === 'cancelled'}
              onclick={() =>
                act(
                  () => sendOffer(id, offer, reason),
                  'Oferta demo gotowa. Email nie jest wysyłany.',
                )}>Zapisz i udostępnij ofertę</Button
            >{/if}{#if order.sandboxPayment}
            <p class="muted">
              Cena zapisanej płatności jest stała. Zwroty wykonaj w panelu Stripe; zakup etykiet nie
              jest jeszcze dostępny.
            </p>
          {:else if order.payment === 'paid'}<Button
              variant="secondary"
              disabled={busy}
              onclick={() => act(() => refundOrder(id), 'Zasymulowano pełny zwrot płatności.')}
              >Symuluj zwrot {money(order.quote.total)}</Button
            >{#if order.delivery !== 'pickup'}<Button
                variant="secondary"
                disabled={busy}
                onclick={() =>
                  act(() => createShippingLabel(id), 'Przygotowano etykietę demonstracyjną.')}
                >Przygotuj etykietę demo</Button
              >{/if}{/if}
          <p class="muted">
            {order.sandboxPayment
              ? 'Status płatności pochodzi ze Stripe. Zmiana statusu produkcji nie zmienia płatności.'
              : 'Status płatności wynika z symulacji płatności, nie z ręcznego oznaczenia przez admina.'}
          </p>
          <p>{order.contact.email} · {order.contact.phone}</p>
          {#if order.contact.company}<p>{order.contact.company} · NIP {order.contact.nip}</p>{/if}
        </section>
      </div>{/if}
    <OrderChat {order} role={adminMode ? 'admin' : 'client'} onchange={assign} />
    <section class="panel stack">
      <h2>Historia zamówienia</h2>
      {#each [...order.history].reverse() as entry}<div class="history">
          <small class="muted">{date(entry.at)}</small>
          <p>{entry.text}</p>
        </div>{/each}
    </section>
  {/if}
</div>

<style>
  .payment-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }
  .configuration {
    border: 0;
    padding-top: 0;
  }
  .configuration summary {
    font-size: 13px;
    color: var(--color-muted);
  }
  .print-settings {
    padding-left: 16px;
    margin: 8px 0;
    color: var(--color-muted);
    font-size: 0.82rem;
    line-height: 1.7;
  }
  .back {
    color: var(--color-muted);
    align-self: start;
    padding: 8px 0;
  }
  .timeline {
    list-style: none;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    padding: 0;
    gap: 12px;
    margin: 0;
  }
  .timeline li {
    display: flex;
    gap: 10px;
    align-items: center;
    color: var(--color-muted);
    font-size: 0.9rem;
  }
  .timeline span {
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    border: 1px solid var(--color-border);
    border-radius: 50%;
    flex-shrink: 0;
  }
  .timeline .active {
    color: var(--color-accent);
  }
  .timeline .active span {
    background: var(--glow);
    border-color: var(--color-primary);
  }
  .model {
    display: grid;
    grid-template-columns: 100px minmax(0, 1fr);
    gap: 18px;
    padding: 16px 0;
  }
  .model + .model {
    border-top: 1px solid var(--color-border);
  }
  .model h3 {
    overflow-wrap: anywhere;
    font-size: 1rem;
  }
  .model p {
    font-size: 0.9rem;
    margin: 6px 0;
  }
  .preview {
    align-self: start;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    background: var(--color-control);
    border-radius: var(--radius-md);
    font-size: 2rem;
    color: var(--color-accent);
    min-height: 105px;
  }
  .preview small {
    font-size: 0.6rem;
    text-align: center;
    color: var(--color-muted);
  }
  dl {
    margin: 0;
    display: grid;
    gap: 12px;
  }
  dl div {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }
  dl dd {
    margin: 0;
    white-space: nowrap;
    flex-shrink: 0;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .total dt {
    font-size: 14px;
    font-weight: 400;
    align-self: center;
  }
  .total {
    font-weight: 600;
    font-size: 24px;
    border-top: 1px solid var(--color-border);
    padding-top: 16px;
  }
  .history p {
    margin: 4px 0;
  }
  .history {
    border-left: 2px solid var(--color-primary);
    padding-left: 14px;
  }
  @media (max-width: 600px) {
    .timeline {
      grid-template-columns: 1fr 1fr;
      gap: 16px;
    }
    .model {
      grid-template-columns: 74px minmax(0, 1fr);
      gap: 12px;
    }
  }
</style>
