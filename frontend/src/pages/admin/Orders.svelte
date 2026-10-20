<script lang="ts">
  import { onMount } from 'svelte';
  import { getOrders } from '../../lib/api';
  import type { Order } from '../../lib/types';
  import { date, money, orderStatus } from '../../lib/format';
  import { navigate } from '../../lib/router';
  import Button from '../../components/ui/Button.svelte';
  import OrderStatus from '../../components/OrderStatus.svelte';
  let orders = $state<Order[]>([]);
  let loading = $state(true);
  let error = $state('');
  let search = $state('');
  let status = $state('');
  let attention = $state(false);
  let filtered = $derived(
    orders.filter(
      (order) =>
        (!status || order.status === status) &&
        (!attention ||
          order.needsAttention ||
          order.messages.some((message) => message.sender === 'client' && !message.read)) &&
        `${order.number} ${order.contact.name} ${order.contact.email} ${order.models.map((model) => model.name).join(' ')}`
          .toLowerCase()
          .includes(search.toLowerCase()),
    ),
  );
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
      <p class="eyebrow">Panel zespołu</p>
      <h1>Zamówienia</h1>
      <p class="muted">Projekty, rozmowy i sprawy wymagające reakcji.</p>
    </div>
    <span class="badge">{orders.filter((order) => order.needsAttention).length} do sprawdzenia</span
    >
  </div>
  <div class="panel filters">
    <label
      >Szukaj<input bind:value={search} type="search" placeholder="Numer, klient, model…" /></label
    ><label
      >Status<select bind:value={status}
        ><option value="">Wszystkie statusy</option
        >{#each Object.entries(orderStatus) as [value, label]}<option {value}>{label}</option
          >{/each}</select
      ></label
    ><label class="check"><input type="checkbox" bind:checked={attention} />Wymaga reakcji</label>
  </div>
  {#if loading}<div class="panel" role="status">Ładowanie zamówień…</div>{:else if error}<div
      class="panel stack"
    >
      <p class="alert error" role="alert">{error}</p>
      <Button onclick={load}>Spróbuj ponownie</Button>
    </div>{:else if !filtered.length}<div class="panel empty-state">
      <h2>Brak zamówień</h2>
      <p>Zmień filtry lub wróć po dodaniu pierwszego zamówienia.</p>
    </div>{:else}<div class="panel table-wrap">
      <table class="data-table" aria-label="Zamówienia">
        <thead
          ><tr><th>Zamówienie</th><th>Klient</th><th>Status</th><th>Kwota</th><th>Akcja</th></tr
          ></thead
        ><tbody
          >{#each filtered as order (order.id)}<tr
              ><td
                ><strong>{order.number}</strong><small>{date(order.createdAt)}</small
                >{#if order.needsAttention}<small class="attention">Wymaga reakcji</small>{/if}</td
              ><td>{order.contact.name}<small>{order.contact.email}</small></td><td
                ><OrderStatus status={order.status} /></td
              ><td>{money(order.quote.total)}</td><td
                ><Button variant="ghost" onclick={() => navigate('/admin/zamowienia/' + order.id)}
                  >Otwórz ↗</Button
                ></td
              ></tr
            >{/each}</tbody
        >
      </table>
    </div>{/if}
</div>

<style>
  .filters {
    display: grid;
    grid-template-columns: 2fr 1fr auto;
    gap: 20px;
    align-items: end;
  }
  .check {
    display: flex;
    gap: 10px;
    align-items: center;
    min-height: 44px;
  }
  .check input {
    width: 20px;
    height: 20px;
    min-height: 20px;
    accent-color: var(--color-primary);
  }
  small {
    display: block;
    color: var(--color-muted);
    margin-top: 6px;
  }
  .attention {
    color: #f1d49b;
  }
  .table-wrap {
    overflow: auto;
  }
  @media (max-width: 760px) {
    .table-wrap {
      padding-top: 0;
    }
    table,
    tbody {
      display: block;
      width: 100%;
    }
    thead {
      position: absolute;
      width: 1px;
      height: 1px;
      clip-path: inset(50%);
      overflow: hidden;
    }
    tbody tr {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 12px 16px;
      padding: 20px 0;
    }
    tbody tr + tr {
      border-top: 1px solid var(--color-border);
    }
    tbody td {
      border: 0;
      padding: 0;
      min-width: 0;
      overflow-wrap: anywhere;
    }
    td:first-child strong {
      white-space: nowrap;
    }
    td:nth-child(2) {
      grid-column: 1 / -1;
      grid-row: 2;
    }
    td:nth-child(3) {
      grid-column: 2;
      grid-row: 1;
      text-align: right;
    }
    td:nth-child(4) {
      align-self: center;
      font-weight: 600;
    }
    td:last-child {
      text-align: right;
    }
    .filters {
      grid-template-columns: 1fr;
    }
  }
</style>
