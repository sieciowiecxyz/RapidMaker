<script lang="ts">
  import { untrack } from 'svelte';
  import {
    listRefunds,
    requestRefund,
    pendingRefund,
    refundPending,
    refundTotals,
    hundredths,
    type Refund,
  } from '../lib/refunds';

  let {
    orderId,
    amountGrosze,
    paid,
    onchange,
  }: {
    orderId: string;
    amountGrosze: number;
    paid: boolean;
    onchange?: () => void;
  } = $props();
  let refunds = $state<Refund[]>([]);
  let amount = $state('');
  let loading = $state(true);
  let busy = $state(false);
  let error = $state('');
  let notice = $state('');
  let uncertain = $state<ReturnType<typeof pendingRefund>>();
  let reload = $state(0);
  let loaded = $state(false);
  const money = (value: number) =>
    new Intl.NumberFormat('pl-PL', {
      style: 'currency',
      currency: 'PLN',
    }).format(value / 100);
  let totals = $derived.by(() => {
    try {
      return refundTotals(refunds, amountGrosze);
    } catch {
      return null;
    }
  });
  let enabled = $derived(
    paid && loaded && !loading && !busy && !!totals && totals.remaining > 0 && !uncertain,
  );
  const labels: Record<string, string> = {
    queued: 'Zlecony',
    pending: 'W realizacji',
    succeeded: 'Zwrócono',
    failed: 'Nieudany',
    canceled: 'Anulowany',
    unknown: 'Sprawdzamy wynik',
    requires_action: 'Wymaga działania',
    manual_review: 'Wymaga sprawdzenia przez operatora',
  };

  $effect(() => {
    const id = orderId;
    reload;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    let attempts = 0;
    untrack(() => {
      loaded = false;
      loading = true;
      refunds = [];
      error = '';
      uncertain = undefined;
    });
    async function refresh() {
      try {
        const result = await listRefunds(id);
        if (!active) return;
        refunds = result;
        loaded = true;
        error = '';
        uncertain = pendingRefund(id);
        if ((result.some(refundPending) || uncertain) && ++attempts < 10)
          timer = setTimeout(refresh, 30_000);
      } catch (cause) {
        if (active)
          error = cause instanceof Error ? cause.message : 'Nie udało się pobrać zwrotów.';
      } finally {
        if (active) loading = false;
      }
    }
    void refresh();
    return () => {
      active = false;
      clearTimeout(timer);
    };
  });

  async function submit(retry = false) {
    if (busy || (!retry && !enabled)) return;
    const id = orderId;
    try {
      const value =
        retry && uncertain ? uncertain.amount_grosze : hundredths(amount, 'Kwota', 99_999_999);
      if (value < 1 || (!retry && (!totals || value > totals.remaining)))
        throw new Error('Kwota musi być dodatnia i nie może przekraczać dostępnych środków.');
      if (
        !retry &&
        !window.confirm(
          `Zlecić zwrot ${money(value)} za to zamówienie? Status realizacji pozostanie bez zmian.`,
        )
      )
        return;
      busy = true;
      error = '';
      await requestRefund(id, value);
      if (orderId !== id) return;
      notice = 'Zlecenie zapisane. Pieniądze zostaną zwrócone po potwierdzeniu przez Stripe.';
      amount = '';
      reload++;
      onchange?.();
    } catch (cause) {
      if (orderId === id) {
        error = cause instanceof Error ? cause.message : 'Nie udało się zlecić zwrotu.';
        uncertain = pendingRefund(id);
      }
    } finally {
      busy = false;
    }
  }
</script>

<section class="refunds" aria-label="Zwroty płatności">
  <header>
    <h3>Zwroty płatności</h3>
    <button class="text-button" disabled={busy || loading} onclick={() => reload++}>Odśwież</button>
  </header>
  {#if loading}<p class="helper" role="status">Sprawdzamy zwroty…</p>
  {:else if totals && loaded}
    <dl>
      <div>
        <dt>Zwrócono</dt>
        <dd>{money(totals.returned)}</dd>
      </div>
      <div>
        <dt>W realizacji</dt>
        <dd>{money(totals.pending)}</dd>
      </div>
      <div>
        <dt>Do zwrotu</dt>
        <dd>{paid ? money(totals.remaining) : 'Brak potwierdzonej płatności'}</dd>
      </div>
    </dl>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if !loading && loaded && !totals}<p class="error" role="alert">
      Kwoty zwrotów wymagają sprawdzenia. Odśwież zamówienie.
    </p>{/if}
  {#if uncertain}
    <p class="helper">
      Nie potwierdzono zlecenia {money(uncertain.amount_grosze)}. Sprawdź status lub ponów tę samą
      operację. Ponowienie nie tworzy drugiego zwrotu.
    </p>
    <button class="secondary" disabled={busy || loading} onclick={() => submit(true)}
      >Ponów to zlecenie</button
    >
  {:else if paid && totals && totals.remaining > 0 && loaded}
    <form
      onsubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <label
        >Kwota zwrotu (zł)<input
          inputmode="decimal"
          autocomplete="off"
          bind:value={amount}
          placeholder="0,00"
          disabled={!enabled}
          required
        /></label
      >
      <button
        type="button"
        class="text-button"
        disabled={!enabled}
        onclick={() => (amount = (totals!.remaining / 100).toFixed(2).replace('.', ','))}
        >Cała pozostała kwota</button
      >
      <button class="secondary" type="submit" disabled={!enabled}>Zleć zwrot</button>
    </form>
  {/if}
  {#if notice}<p class="helper" role="status">{notice}</p>{/if}
  {#if refunds.length}
    <ul>
      {#each refunds as refund (refund.id)}<li>
          <div>
            <strong>{money(refund.amount_grosze)}</strong><small
              >{new Date(refund.created_at * 1000).toLocaleString('pl-PL')}</small
            >
          </div>
          <span>{labels[refund.state] ?? 'Wymaga sprawdzenia'}</span>
        </li>{/each}
    </ul>
  {:else if loaded && !loading}<p class="helper">Brak zleconych zwrotów.</p>{/if}
  <p class="helper">Zwrot nie anuluje zamówienia ani przesyłki. Status potwierdza serwer.</p>
</section>

<style>
  .refunds {
    margin-top: 24px;
    display: grid;
    gap: 16px;
    font-size: 12px;
  }
  header,
  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  dl {
    display: flex;
    flex-wrap: wrap;
    gap: 16px 28px;
    margin: 0;
  }
  dt,
  .helper {
    color: var(--muted);
  }
  dd {
    margin: 4px 0 0;
    font-variant-numeric: tabular-nums;
  }
  form {
    display: flex;
    align-items: flex-end;
    flex-wrap: wrap;
    gap: 12px;
  }
  label {
    display: grid;
    gap: 6px;
    max-width: 180px;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  li {
    padding: 12px 0;
  }
  li + li {
    border-top: 1px solid var(--line);
  }
  li > span {
    color: var(--muted);
    text-align: right;
  }
  .error {
    color: var(--danger, #ffb4a9);
  }
  .helper {
    line-height: 1.6;
  }
</style>
