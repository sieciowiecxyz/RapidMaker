<script lang="ts">
  import { IconArrowUpRight, IconSearch } from '@tabler/icons-svelte';
  import { total, money, shortId, type Order } from './data';
  let {
    orders,
    mode,
    selected = null,
    onopen,
  }: {
    orders: Order[];
    mode: string;
    selected?: Order | null;
    onopen: (o: Order, e?: MouseEvent) => void;
  } = $props();
  let search = $state('');
  let filter = $state('Wszystkie');
  let shown = $derived(
    orders
      .filter((o) =>
        (o.id + ' ' + o.client).toLocaleLowerCase('pl').includes(search.toLocaleLowerCase('pl')),
      )
      .filter((o) => filter === 'Wszystkie' || (filter === 'Opłacone' ? o.paid : !o.paid)),
  );
</script>

{#if mode === 'Faktury'}
  <section class="invoice-section">
    <h3>Faktury</h3>
    <p class="helper">Wystawianie faktur nie jest jeszcze obsługiwane przez serwer.</p>
    <button class="primary" disabled>Wystaw fakturę</button>
  </section>
{:else if selected}
  <div class="finance-payment">
    <div>
      <h3>Płatność za zamówienie</h3>
      <p class="helper">Całe zamówienie: <strong>{money(total(selected))}</strong></p>
    </div>
    <span class="status" data-status={selected.paid ? 'complete' : 'waiting'}
      >{selected.payment}</span
    >
  </div>
  <p class="helper">
    Status płatności potwierdza serwer. Ręczne oznaczanie zamówienia jako opłaconego jest
    niedostępne.
  </p>
  <section class="invoice-section">
    <h3>Faktura</h3>
    <p class="helper">Wystawianie dokumentów nie jest jeszcze obsługiwane przez serwer.</p>
    <button class="secondary" disabled>Wystaw fakturę</button>
  </section>
{:else}
  <div class="finance-summary">
    <span
      >Oczekuje na płatność <strong
        >{money(orders.filter((o) => !o.paid).reduce((s, o) => s + total(o), 0))}</strong
      ></span
    ><span
      >Potwierdzone płatności <strong
        >{money(orders.filter((o) => o.paid).reduce((s, o) => s + total(o), 0))}</strong
      ></span
    ><span class="muted">Kwoty całych zamówień · PLN</span>
  </div>
  <div class="list-tools">
    <div class="filters" aria-label="Filtr płatności">
      {#each ['Wszystkie', 'Nieopłacone', 'Opłacone'] as f}<button
          class:chosen={filter === f}
          aria-pressed={filter === f}
          onclick={() => (filter = f)}>{f}</button
        >{/each}
    </div>
    <label class="search"
      ><IconSearch size={18} /><input
        aria-label="Szukaj płatności"
        bind:value={search}
        placeholder="Szukaj zamówienia lub klienta…"
      /></label
    >
  </div>
  <div class="table-scroll">
    <table>
      <thead
        ><tr
          ><th>Zamówienie / klient</th><th>Płatność</th><th>Źródło</th><th class="amount"
            >Kwota zamówienia</th
          ><th>Akcja</th></tr
        ></thead
      ><tbody
        >{#each shown as o}<tr
            ><td
              ><button class="order-link" onclick={(e) => onopen(o, e)}
                >{shortId(o.id)}<IconArrowUpRight size={14} /></button
              ><strong class="client">{o.client}</strong></td
            ><td
              ><span class="status" data-status={o.paid ? 'complete' : 'waiting'}>{o.payment}</span
              ></td
            ><td><span class="muted">Potwierdzenie serwera</span></td><td class="amount"
              >{money(total(o))}</td
            ><td
              ><button class="text-button" onclick={(e) => onopen(o, e)}
                >Szczegóły rozliczenia</button
              ></td
            ></tr
          >{/each}</tbody
      >
    </table>
    {#if !shown.length}<div class="empty">
        <h2>Brak pasujących płatności</h2>
        <button
          class="secondary"
          onclick={() => {
            search = '';
            filter = 'Wszystkie';
          }}>Wyczyść filtry</button
        >
      </div>{/if}
  </div>
{/if}
