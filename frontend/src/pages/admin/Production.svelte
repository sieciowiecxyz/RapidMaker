<script lang="ts">
  import { onMount } from 'svelte';
  import { getOrders, updateOrder } from '../../lib/api';
  import type { Order, OrderStatus as Status } from '../../lib/types';
  import Button from '../../components/ui/Button.svelte';
  import OrderStatus from '../../components/OrderStatus.svelte';
  let orders = $state<Order[]>([]);
  let loading = $state(true);
  let error = $state('');
  let busy = $state('');
  const stages: { id: Status; label: string; next: Status }[] = [
    { id: 'preparing', label: 'Przygotowanie', next: 'printing' },
    { id: 'printing', label: 'W druku', next: 'ready' },
    { id: 'ready', label: 'Gotowe', next: 'completed' },
  ];
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
  async function advance(order: Order, next: Status) {
    busy = order.id;
    error = '';
    try {
      const updated = await updateOrder(order.id, {
        status: next === 'shipped' && order.delivery === 'pickup' ? 'ready' : next,
      });
      orders = orders.map((item) => (item.id === updated.id ? updated : item));
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = '';
    }
  }
</script>

<div class="page stack">
  <div class="page-heading">
    <div>
      <p class="eyebrow">Panel zespołu</p>
      <h1>Produkcja</h1>
      <p class="muted">Kolejka wydruków i status realizacji.</p>
    </div>
  </div>
  {#if error}<p class="alert error" role="alert">{error}</p>{/if}{#if loading}<div
      class="panel"
      role="status"
    >
      Ładowanie kolejki…
    </div>{:else if error && !orders.length}<Button onclick={load}>Spróbuj ponownie</Button
    >{:else}<div class="board">
      {#each stages as stage}<section class="column">
          <div class="toolbar">
            <h2>{stage.label}</h2>
            <span class="badge">{orders.filter((order) => order.status === stage.id).length}</span>
          </div>
          {#each orders.filter((order) => order.status === stage.id) as order (order.id)}<article
              class="production-job stack"
            >
              <div class="toolbar">
                <strong>{order.number}</strong><OrderStatus status={order.status} />
              </div>
              <h3>{order.models[0]?.name}</h3>
              <p class="muted">
                {order.contact.name} · {order.models.reduce(
                  (sum, model) => sum + model.options.quantity,
                  0,
                )} szt.
              </p>
              <p>Termin: {order.leadTime}</p>
              {#if order.notes}<p class="note">{order.notes}</p>{/if}<a
                class="job-link"
                href={'/admin/zamowienia/' + order.id}>Szczegóły →</a
              ><Button
                disabled={busy === order.id ||
                  (stage.next === 'completed' && order.delivery !== 'pickup')}
                onclick={() => advance(order, stage.next)}
                >{stage.next === 'printing'
                  ? 'Rozpocznij druk'
                  : stage.next === 'completed'
                    ? 'Potwierdź odbiór'
                    : order.delivery === 'pickup'
                      ? 'Gotowe do odbioru'
                      : 'Przekaż do wysyłki'}</Button
              >
            </article>{:else}<p class="empty-state">Brak zleceń na tym etapie.</p>{/each}
        </section>{/each}
    </div>{/if}
</div>

<style>
  .board {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 20px;
    align-items: start;
  }
  .column {
    display: grid;
    gap: 16px;
    min-width: 0;
  }
  .column > .empty-state {
    padding: 12px 0;
    text-align: left;
    font-size: 13px;
  }
  .column > .toolbar h2 {
    margin: 0;
  }
  .production-job {
    border-top: 1px solid var(--color-border);
    padding: 20px 0;
    gap: 12px;
  }
  .production-job h3 {
    margin: 0;
  }
  .job-link {
    color: var(--color-accent);
    min-height: 44px;
    display: flex;
    align-items: center;
    font-size: 13px;
  }
  h2 {
    font-size: 1.1rem;
  }
  h3 {
    font-size: 1rem;
    overflow-wrap: anywhere;
  }
  p {
    margin: 0;
  }
  .note {
    font-size: 0.9rem;
    padding: 12px;
    background: var(--color-control);
    border-radius: var(--radius-sm);
    white-space: pre-wrap;
  }
  @media (max-width: 1000px) {
    .board {
      grid-template-columns: 1fr;
    }
  }
</style>
