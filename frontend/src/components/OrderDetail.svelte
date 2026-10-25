<script lang="ts">
  import { tick } from 'svelte';
  import {
    getOrder,
    updateOrder,
    refundOrder,
    refreshOrderPayment,
    openOrderPayment,
  } from '../lib/api';
  import {
    money,
    date,
    orderStatus,
    paymentStatus,
    printConfiguration,
    parcels,
  } from '../lib/format';
  import type { Order, OrderStatus as Status } from '../lib/types';
  import type { LiveOrder } from '../lib/orders';
  import { ApiError } from '../lib/http';
  import { navigate } from '../lib/router';
  import Button from './ui/Button.svelte';
  import OrderStatus from './OrderStatus.svelte';
  import OrderChat from './OrderChat.svelte';
  import ParcelPlan from './ParcelPlan.svelte';
  import ModelPreview from './ModelPreview.svelte';
  import ManualOffer from './ManualOffer.svelte';
  let { id, adminMode = false }: { id: string; adminMode?: boolean } = $props();
  let order = $state<LiveOrder | null>(null);
  let loading = $state(true);
  let loadGeneration = 0;
  let busy = $state(false);
  let error = $state('');
  let success = $state('');
  let status = $state<Status>('preparing');
  let leadTime = $state('');
  let notes = $state('');
  function assign(value: Order) {
    order = value;
    status = value.status;
    leadTime = value.leadTime;
    notes = value.notes;
  }
  async function load() {
    const generation = ++loadGeneration;
    order = null;
    loading = true;
    error = '';
    try {
      const value = await getOrder(id);
      if (generation !== loadGeneration) return;
      assign(value);
    } catch (e) {
      if (generation !== loadGeneration) return;
      error = (e as Error).message;
    } finally {
      if (generation === loadGeneration) loading = false;
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
    return () => {
      loadGeneration++;
    };
  });
  async function act(action: () => Promise<Order>, message: string) {
    busy = true;
    error = '';
    success = '';
    try {
      assign(await action());
      success = message;
    } catch (e) {
      if (e instanceof ApiError && (e.status === 401 || e.status === 403)) order = null;
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
  const nextStatus = $derived(
    order?.serverStatus === 'paid'
      ? 'printing'
      : order?.serverStatus === 'in_production'
        ? 'ready'
        : order?.serverStatus === 'ready' && order.delivery === 'pickup'
          ? 'completed'
          : null,
  );
  async function refund() {
    if (
      !order ||
      !confirm(
        `Zlecić zwrot ${money(order.refundableAmount ?? 0)}? Status produkcji pozostanie bez zmian.`,
      )
    )
      return;
    await act(() => refundOrder(id), 'Zlecenie zwrotu zapisane. Sprawdź jego status poniżej.');
  }
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
        <h1 title={order.id}>{order.number}</h1>
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
        {#each order.models as model, index (index)}<article class="model">
            <div class="preview" aria-label="Format pliku">
              <strong>{model.format}</strong><small>Plik 3D</small>
            </div>
            <div>
              <h3>{model.name}</h3>
              <p class="muted">{model.dimensions} · {model.weight}</p>
              <p>{model.options.materialId.toUpperCase()} · {model.options.color}</p>
              <p class="muted">{model.options.quantity} szt.</p>
              <details class="configuration">
                <summary>Parametry wydruku</summary>
                <ul class="print-settings">
                  {#if order.serverStatus}{#if order.files?.filter((f) => f.model_id === model.id).length === 1 && model.advanced.wallCount > 0}<li
                      >
                        Jakość: {{ draft: 'robocza', standard: 'standardowa', fine: 'dokładna' }[
                          model.options.quality
                        ]}
                      </li>
                      <li>
                        Ściany: {model.advanced.wallCount}; wypełnienie: {model.advanced.infill}%
                      </li>{:else}<li>
                        Dokładne ustawienia znajdują się w plikach zamówienia.
                      </li>{/if}{:else}{#each printConfiguration(model) as setting}<li>
                        {setting}
                      </li>{/each}{/if}
                </ul>
              </details>
              {#if model.notes}<p>Uwagi: {model.notes}</p>{/if}
            </div>
          </article>{/each}{#if !adminMode}<Button variant="secondary" disabled
            >Zamów ponownie ↗</Button
          ><small class="muted">Ponowne zamawianie nie jest jeszcze dostępne.</small>{/if}
        {#if order.files?.length}<div class="stack">
            <h3>Pliki zamówienia</h3>
            {#each order.files as file}<div>
                {file.filename}
                {#if file.available && file.preview_url?.startsWith(`/api/${adminMode ? 'admin' : 'account'}/orders/${order.id}/files/`)}
                  <ModelPreview
                    url={file.preview_url}
                    name={file.filename}
                    openInitially={order.files.length === 1}
                  />
                {/if}
                {#if file.available && file.source_url?.startsWith(`/api/${adminMode ? 'admin' : 'account'}/orders/${order.id}/files/`)}<a
                    href={file.source_url}
                    download={file.filename}>Pobierz oryginał ↗</a
                  >{:else}<small class="muted">Plik nie jest już dostępny</small>{/if}
              </div>{/each}
          </div>{/if}
      </section>
      <section id="platnosc" class="panel stack">
        <h2>Płatność i dostawa</h2>
        {#if order.manualOffer}<ManualOffer
            orderId={order.id}
            offer={order.manualOffer}
            {adminMode}
            onchange={async () => {
              if (!adminMode && order?.serverStatus === 'awaiting_payment') {
                await refreshOrderPayment(id);
              }
              await load();
            }}
          />{/if}
        <dl>
          <div>
            <dt>
              {order.serverStatus
                ? order.deliveryKnown
                  ? 'Wydruki i opakowania'
                  : 'Zapisana kwota zamówienia'
                : 'Wydruki'}
            </dt>
            <dd>{money(order.quote.subtotal)}</dd>
          </div>
          {#if order.quote.checkout?.packaging_grosze}
            <div>
              <dt>Opakowania</dt>
              <dd>{money(order.quote.checkout.packaging_grosze / 100)}</dd>
            </div>
          {/if}
          <div>
            <dt>
              Dostawa{#if !order.serverStatus || order.deliveryKnown}
                · {parcels(order.quote.packages)}{/if}
            </dt>
            <dd>
              {order.manualOffer?.shipping_grosze === null
                ? 'Do wyceny'
                : order.serverStatus && !order.deliveryKnown
                  ? 'Brak zapisanych danych'
                  : money(order.quote.delivery)}
            </dd>
          </div>
          <div class="total">
            <dt>{order.quote.manual ? 'Szacunek do potwierdzenia' : 'Razem'}</dt>
            <dd>{money(order.quote.total)}</dd>
          </div>
        </dl>
        <p class="badge">{paymentStatus[order.payment]}</p>
        {#if order.serverStatus && !order.manualOffer}
          <div class="payment-actions">
            {#if !adminMode && order.paymentReference && !['paid', 'refunded'].includes(order.payment) && order.status !== 'cancelled'}
              <Button disabled={busy} onclick={pay}>Przejdź do płatności ↗</Button>
            {/if}
            <Button
              variant="secondary"
              disabled={busy || adminMode}
              onclick={() =>
                act(() => refreshOrderPayment(id), 'Sprawdzono status płatności w Stripe.')}
              >Sprawdź płatność</Button
            >
          </div>
          <small class="muted">Status płatności potwierdza serwer.</small>
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
          <small class="muted">Numer przesyłki przewoźnika.</small>{/if}{#if order.label}<p
            class="badge"
          >
            {order.label}
          </p>{/if}
        {#if order.quote.checkout}
          <ParcelPlan
            packing={order.quote.checkout.packing}
            models={order.models}
            delivery={order.delivery}
          />
        {/if}
        {#if order.serverStatus && !order.manualOffer && !order.paymentReference && order.payment !== 'paid' && order.payment !== 'refunded'}<p
            class="muted"
          >
            Wznowienie płatności w tym widoku nie jest jeszcze dostępne. Status płatności potwierdza
            serwer.
          </p>{/if}
        {#if order.quote.manual}<p class="alert">
            Zlecenie wymaga ręcznej wyceny. Ustalimy cenę oraz sposób dostawy w rozmowie.
          </p>{/if}{#if order.offerReason && !order.manualOffer}<p class="alert">
            Oferta obsługi: {order.offerReason}
          </p>{/if}
      </section>
    </div>
    {#if adminMode}<div class="grid-2">
        <section class="panel stack">
          <h2>Produkcja</h2>
          <label for="order-status">Status</label><select
            id="order-status"
            bind:value={status}
            disabled={!nextStatus || busy}
            ><option value={order.status}>{orderStatus[order.status]}</option
            >{#if nextStatus}<option value={nextStatus}>{orderStatus[nextStatus]}</option
              >{/if}</select
          >
          <label for="lead-time">Termin realizacji</label><input
            id="lead-time"
            value={leadTime}
            disabled
          /><label for="private-notes">Prywatne notatki zespołu</label><textarea
            id="private-notes"
            value={notes}
            rows="3"
            disabled></textarea><Button
            disabled={busy || !nextStatus || status === order.status}
            onclick={() => act(() => updateOrder(id, { status }), 'Status realizacji zapisany.')}
            >Zapisz status</Button
          ><small class="muted">Terminy i notatki wewnętrzne nie są jeszcze obsługiwane.</small>
        </section>
        <section class="panel stack">
          <h2>Płatność i kontakt</h2>
          {#if order.payment === 'paid'}<Button
              variant="secondary"
              disabled={busy || order.refundPending || !order.refundableAmount}
              onclick={refund}>Zleć zwrot {money(order.refundableAmount ?? 0)}</Button
            >{/if}
          {#if order.refundNotice}<p class="alert" role="status">{order.refundNotice}</p>{/if}
          {#if order.delivery !== 'pickup'}<Button variant="secondary" disabled
              >Przygotuj etykietę</Button
            ><small class="muted"
              >Nadanie wymaga osobnego potwierdzenia paczek i kosztów; ten widok jeszcze tego nie
              obsługuje.</small
            >{/if}
          <p class="muted">
            Cenę i płatność potwierdza serwer. Zwrot nie zmienia statusu produkcji. Ręczna zmiana
            ceny nie jest dostępna.
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
  .page-heading > div {
    min-width: 0;
  }
  .page-heading h1 {
    overflow-wrap: anywhere;
  }
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
