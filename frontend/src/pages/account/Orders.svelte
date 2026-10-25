<script lang="ts">
  import { onMount } from 'svelte';
  import { getOrders } from '../../lib/api';
  import type { Order } from '../../lib/types';
  import { money, date } from '../../lib/format';
  import Button from '../../components/ui/Button.svelte';
  import OrderStatus from '../../components/OrderStatus.svelte';
  import { navigate } from '../../lib/router';
  let orders = $state<Order[]>([]);
  let loading = $state(true);
  let error = $state('');
  let busy = $state('');
  async function load() {
    loading = true;
    error = '';
    try {
      orders = await getOrders();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      loading = false;
    }
  }
  onMount(load);
</script>

<div class="page stack">
  <div class="page-heading">
    <div>
      <p class="eyebrow">Twoja przestrzeń</p>
      <h1>Moje zamówienia</h1>
      <p class="muted">Od projektu do paczki. Wszystko w jednym miejscu.</p>
    </div>
    <Button onclick={() => navigate('/wycena')}>Nowy wydruk ↗</Button>
  </div>
  {#if error}<p class="alert error" role="alert">{error}</p>{/if}
  {#if loading}<div class="panel" role="status">Ładowanie zamówień…</div>
  {:else if error && !orders.length}<div>
      <Button variant="secondary" onclick={load}>Spróbuj ponownie</Button>
    </div>
  {:else if !orders.length}<div class="panel empty-state">
      <h2>Twój pierwszy projekt czeka</h2>
      <p>Dodaj model i sprawdź wycenę wydruku.</p>
      <Button onclick={() => navigate('/wycena')}>Dodaj model</Button>
    </div>
  {:else}
    <ul class="panel orders" aria-label="Twoje zamówienia">
      {#each orders as order (order.id)}
        {@const unread = order.messages.filter(
          (message) => message.sender === 'admin' && !message.read,
        ).length}
        {@const detail = '/konto/zamowienia/' + order.id}
        <li class="order-row" aria-labelledby={`number-${order.id}`} aria-busy={busy === order.id}>
          <div class="order-info">
            <div class="identity">
              <strong id={`number-${order.id}`} title={order.id}>{order.number}</strong><OrderStatus
                status={order.status}
              />
            </div>
            <h2>
              {order.models[0]?.name}{order.models.length > 1
                ? ` + ${order.models.length - 1}`
                : ''}
            </h2>
            <p class="muted">
              {date(order.createdAt)} · {order.models.length}
              {order.models.length === 1 ? 'model' : 'modele'} · {order.models.reduce(
                (sum, model) => sum + model.options.quantity,
                0,
              )} szt.
            </p>
          </div>
          <div class="row-right">
            <strong class="price">{money(order.quote.total)}</strong>
            <div class="actions">
              <a
                class="button secondary"
                href={detail}
                aria-label={`Szczegóły zamówienia ${order.number}`}>Szczegóły ↗</a
              >
              <a class="button ghost messages" href={`${detail}#rozmowa`}
                >Wiadomości{#if unread}<span class="unread">{unread}</span>{/if}</a
              >
            </div>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .orders {
    margin: 0;
    padding: 0 24px;
    list-style: none;
  }
  .order-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    padding: 24px 0;
  }
  .order-row + .order-row {
    border-top: 1px solid var(--color-border);
  }
  .order-info {
    min-width: 0;
    flex: 1;
  }
  .identity {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }
  .identity strong {
    font-size: 14px;
    letter-spacing: 0.02em;
  }
  h2 {
    font-size: 15px;
    font-weight: 500;
    margin: 10px 0 7px;
    overflow-wrap: anywhere;
  }
  p {
    margin: 0;
    font-size: 12px;
  }
  .row-right {
    display: flex;
    align-items: center;
    gap: 28px;
  }
  .price {
    font-size: 16px;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
    min-width: 100px;
    text-align: right;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .actions :global(.button) {
    font-size: 12px;
    min-height: 44px;
    padding: 10px 12px;
    white-space: nowrap;
  }
  .unread {
    display: inline-grid;
    place-items: center;
    min-width: 19px;
    height: 19px;
    border-radius: 50%;
    background: var(--color-primary);
    color: var(--color-on-primary);
    font-size: 11px;
  }
  @media (max-width: 1100px) {
    .order-row {
      flex-wrap: wrap;
      gap: 18px;
    }
    .order-info {
      flex-basis: 60%;
    }
    .row-right {
      width: 100%;
      justify-content: space-between;
    }
    .price {
      text-align: left;
    }
  }
  @media (max-width: 600px) {
    .orders {
      padding: 0 16px;
    }
    .order-row {
      padding: 22px 0;
    }
    .row-right {
      align-items: flex-start;
      flex-direction: column;
      gap: 12px;
    }
    .actions {
      flex-wrap: wrap;
      gap: 4px;
      width: 100%;
    }
    .actions :global(.button) {
      padding: 10px 11px;
    }
    .identity {
      gap: 10px;
    }
  }
</style>
