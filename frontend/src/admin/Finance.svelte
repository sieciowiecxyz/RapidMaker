<script lang="ts">
  import { untrack } from 'svelte';
  import {
    IconArrowUpRight,
    IconSearch,
    IconFileInvoice,
    IconArrowRight,
  } from '@tabler/icons-svelte';
  import { total, money, shortId, type Order } from './data';
  import { saveInvoice, type InvoiceDraft } from './production';
  let {
    orders,
    mode,
    selected = null,
    onopen,
    onsaved = () => {},
  }: {
    orders: Order[];
    mode: string;
    selected?: Order | null;
    onopen: (o: Order, e?: MouseEvent) => void;
    onsaved?: () => void | Promise<void>;
  } = $props();
  let search = $state(''),
    filter = $state('Wszystkie'),
    message = $state(''),
    error = $state(''),
    busy = $state(false);
  let draft = $state<InvoiceDraft | null>(null),
    active = '',
    revision = -1;
  const invoiceMode = $derived(mode === 'Faktury');
  const shown = $derived(
    orders
      .filter((o) =>
        (o.id + ' ' + o.client).toLocaleLowerCase('pl').includes(search.toLocaleLowerCase('pl')),
      )
      .filter(
        (o) =>
          filter === 'Wszystkie' ||
          (invoiceMode
            ? filter === 'Szkice'
              ? !!o.invoiceDraft
              : !o.invoiceDraft
            : filter === 'Opłacone'
              ? o.paid
              : !o.paid),
      ),
  );
  const snapshot = (o: Order): InvoiceDraft => ({
    revision: 0,
    buyer: o.invoiceContact.buyer,
    address: o.invoiceContact.address,
    nip: o.invoiceContact.nip,
    created: new Date().toISOString().slice(0, 10),
    lines: [
      {
        name: 'Druk 3D — zamówienie ' + shortId(o.id) + ' (z przygotowaniem i dostawą)',
        qty: 1,
        price_grosze: o.amountGrosze ?? 0,
      },
    ],
  });
  const sum = (d: InvoiceDraft) => d.lines.reduce((n, l) => n + l.qty * l.price_grosze, 0);
  $effect(() => {
    const id = selected?.id ?? '',
      next = selected?.invoiceDraft?.revision ?? 0;
    untrack(() => {
      if (id !== active || next !== revision) {
        active = id;
        revision = next;
        draft = selected
          ? structuredClone($state.snapshot(selected.invoiceDraft ?? snapshot(selected)))
          : null;
        message = '';
        error = '';
      }
    });
  });
  async function save() {
    if (!selected || !draft || busy) return;
    busy = true;
    error = '';
    message = '';
    try {
      draft = await saveInvoice(selected.id, draft);
      await onsaved();
      message = 'Szkic zapisany na serwerze. Dokument nie został wystawiony.';
    } catch (e) {
      error = e instanceof Error ? e.message : 'Nie udało się zapisać szkicu.';
    } finally {
      busy = false;
    }
  }
  function refresh() {
    if (!selected || !draft) return;
    draft = { ...draft, lines: snapshot(selected).lines };
    message = 'Pozycje odświeżone z pełnej kwoty zamówienia. Zapisz szkic, aby je zachować.';
  }
</script>

{#if selected && draft}
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
    Status płatności potwierdza Stripe. Nie można ręcznie oznaczyć zamówienia jako opłaconego.
  </p>
  <section class="invoice-section">
    <h3>Faktura do {shortId(selected.id)}</h3>
    {#if selected.invoiceDraft}
      <div class="invoice-state">
        <span class="status" data-status="preparing">Szkic</span><span class="muted"
          >Dokument niewystawiony · wersja {selected.invoiceDraft.revision}</span
        >
      </div>
      <form
        onsubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <label class="message-label"
          >Nabywca<input bind:value={draft.buyer} required maxlength="200" disabled={busy} /></label
        >
        <label class="message-label"
          >Adres nabywcy<textarea
            bind:value={draft.address}
            rows="2"
            maxlength="500"
            disabled={busy}></textarea></label
        >
        <label class="message-label"
          >NIP (opcjonalnie)<input
            bind:value={draft.nip}
            maxlength="10"
            inputmode="numeric"
            disabled={busy}
          /></label
        >
        <label class="message-label"
          >Data szkicu<input
            type="date"
            bind:value={draft.created}
            required
            disabled={busy}
          /></label
        >
        <div class="invoice-lines">
          {#each draft.lines as line}<div>
              <span
                >{line.name}<small>{line.qty} szt. × {money(line.price_grosze / 100)}</small></span
              ><strong>{money((line.qty * line.price_grosze) / 100)}</strong>
            </div>{/each}
        </div>
        <div class="invoice-total">
          <span>Suma pozycji szkicu brutto</span><strong>{money(sum(draft) / 100)}</strong>
        </div>
        {#if sum(draft) !== selected.amountGrosze}<p class="warning">
            Kwota zamówienia zmieniła się od utworzenia szkicu.
          </p>
          <button type="button" class="secondary" disabled={busy} onclick={refresh}
            >Aktualizuj z zamówienia</button
          >{/if}
        <div class="file-actions">
          <button class="primary" disabled={busy}>Zapisz szkic</button><button
            type="button"
            class="secondary"
            disabled
            title="Wymaga systemu fakturowania">Wystaw fakturę</button
          >
        </div>
      </form>
      <p class="helper">
        To robocze dane, bez numeru faktury, księgowania i PDF. Podatki i wystawienie wymagają
        systemu fakturowania.
      </p>
    {:else}
      <div class="invoice-empty">
        <IconFileInvoice size={32} />
        <h3>Brak dokumentu</h3>
        <p class="helper">
          Przygotuj szkic z danych nabywcy i pełnej ceny zamówienia, łącznie z przygotowaniem i
          dostawą.
        </p>
        <button class="primary" disabled={busy || selected.amountGrosze === null} onclick={save}
          >Utwórz szkic faktury<IconArrowRight size={16} /></button
        >
      </div>
    {/if}
    {#if error}<p class="warning" role="alert">{error}</p>{/if}{#if message}<p
        class="helper"
        role="status"
      >
        {message}
      </p>{/if}
  </section>
{:else}
  <div class="finance-summary">
    {#if invoiceMode}<span
        >Do przygotowania <strong>{orders.filter((o) => !o.invoiceDraft).length}</strong></span
      ><span>Zapisane szkice <strong>{orders.filter((o) => o.invoiceDraft).length}</strong></span>
    {:else}<span
        >Oczekuje na płatność <strong
          >{money(
            orders.filter((o) => !o.paid && o.amount !== null).reduce((s, o) => s + total(o)!, 0),
          )}</strong
        ></span
      ><span
        >Potwierdzone płatności <strong
          >{money(
            orders.filter((o) => o.paid && o.amount !== null).reduce((s, o) => s + total(o)!, 0),
          )}</strong
        ></span
      >{/if}
    <span class="muted"
      >{invoiceMode
        ? 'Szkice robocze · bez wystawiania dokumentów'
        : 'Kwoty zamówień potwierdzane przez serwer · PLN'}</span
    >
  </div>
  <div class="list-tools">
    <div class="filters" aria-label={invoiceMode ? 'Filtr dokumentów' : 'Filtr płatności'}>
      {#each invoiceMode ? ['Wszystkie', 'Do przygotowania', 'Szkice'] : ['Wszystkie', 'Nieopłacone', 'Opłacone'] as f}<button
          class:chosen={filter === f}
          aria-pressed={filter === f}
          onclick={() => (filter = f)}>{f}</button
        >{/each}
    </div>
    <label class="search"
      ><IconSearch size={18} /><input
        aria-label={invoiceMode ? 'Szukaj dokumentów' : 'Szukaj płatności'}
        bind:value={search}
        placeholder="Szukaj zamówienia lub klienta…"
      /></label
    >
  </div>
  <div class="table-scroll">
    <table>
      <thead
        ><tr
          ><th>Zamówienie / klient</th><th>{invoiceMode ? 'Dokument' : 'Płatność'}</th><th
            >Źródło</th
          ><th class="amount">Kwota zamówienia</th><th>Akcja</th></tr
        ></thead
      ><tbody>
        {#each shown as o}<tr
            ><td
              ><button class="order-link" onclick={(e) => onopen(o, e)}
                >{shortId(o.id)}<IconArrowUpRight size={14} /></button
              ><strong class="client">{o.client}</strong></td
            >
            <td
              ><span
                class="status"
                data-status={invoiceMode
                  ? o.invoiceDraft
                    ? 'preparing'
                    : 'quote'
                  : o.paid
                    ? 'complete'
                    : 'waiting'}
                >{invoiceMode ? (o.invoiceDraft ? 'Szkic' : 'Brak dokumentu') : o.payment}</span
              ></td
            >
            <td
              ><span class="muted"
                >{invoiceMode ? 'Dokument niewystawiony' : 'Potwierdzenie serwera'}</span
              ></td
            ><td class="amount">{money(total(o))}</td><td
              ><button class="text-button" onclick={(e) => onopen(o, e)}
                >{invoiceMode
                  ? o.invoiceDraft
                    ? 'Otwórz szkic'
                    : 'Przygotuj szkic'
                  : 'Szczegóły rozliczenia'}</button
              ></td
            ></tr
          >{/each}
      </tbody>
    </table>
    {#if !shown.length}<div class="empty">
        <h2>Brak pasujących pozycji</h2>
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
