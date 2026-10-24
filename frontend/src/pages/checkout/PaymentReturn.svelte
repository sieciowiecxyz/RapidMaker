<script lang="ts">
  import RetryPayment from './RetryPayment.svelte';
  import type { Order, StripeStatus } from '../../lib/types';
  import { money } from '../../lib/format';
  let {
    payment,
    receipt,
    order,
  }: {
    payment: StripeStatus;
    order: Order | null;
    receipt: { items: { name: string; quantity: number }[]; delivery: string } | null;
  } = $props();
  let retriedStatus = $state<StripeStatus | null>(null);
  const current = $derived(retriedStatus ?? payment);
  const paid = $derived(current.status === 'paid');
  const intentId = $derived(
    order?.sandboxPayment?.sessionId ??
      new URLSearchParams(window.location.search).get('payment_intent') ??
      new URLSearchParams(window.location.search).get('stripe_session') ??
      '',
  );
</script>

<section class="payment-result panel stack" aria-labelledby="payment-result-title">
  <span class:paid class="result-icon" aria-hidden="true">{paid ? '✓' : '↗'}</span>
  <span class="eyebrow">Płatność testowa</span>
  <h2 id="payment-result-title">
    {paid
      ? 'Płatność potwierdzona.'
      : current.checkout_status === 'expired'
        ? 'Sesja płatności wygasła.'
        : 'Płatność jeszcze przed Tobą.'}
  </h2>
  <p class="muted">
    {paid
      ? 'Stripe potwierdził wpłatę. Poniżej znajdziesz podsumowanie.'
      : 'Nie otrzymaliśmy potwierdzenia wpłaty. Możesz bezpiecznie wrócić do płatności.'}
  </p>
  {#if order}<strong>Zamówienie {order.number}</strong>{/if}
  {#if receipt}
    <ul>
      {#each receipt.items as item}<li>
          <span>{item.name}</span><strong>{item.quantity} szt.</strong>
        </li>{/each}
      <li><span>Dostawa</span><strong>{receipt.delivery}</strong></li>
    </ul>
  {/if}
  <div class="amount">
    <span>{paid ? 'Zapłacono' : 'Do zapłaty'}</span><strong
      >{money(current.total_grosze / 100)}</strong
    >
  </div>
  {#if !paid && current.checkout_status === 'open' && payment.url.startsWith('https://checkout.stripe.com/')}
    <a class="button primary" href={payment.url}>Wróć do płatności →</a>
  {/if}
  {#if !paid && current.checkout_status === 'open' && intentId.startsWith('pi_')}
    <RetryPayment
      {intentId}
      amountGrosze={current.total_grosze}
      onstatus={(status) => (retriedStatus = status)}
    />
  {/if}
  <a class="button secondary" href="/wycena">{paid ? 'Dodaj kolejny projekt' : 'Wróć do wyceny'}</a>
  {#if order}
    <a
      class="button secondary"
      href={order.ownerId
        ? '/konto/zamowienia'
        : '/logowanie?register=1&email=' + encodeURIComponent(order.contact.email)}
      >{order.ownerId ? 'Szczegóły zamówienia →' : 'Utwórz konto i śledź zamówienie →'}</a
    >
  {/if}
  <small class="muted">Numer wyceny: {payment.quote_id.slice(0, 8).toUpperCase()}</small>
</section>

<style>
  .payment-result {
    max-width: 620px;
    margin: 24px auto;
  }
  .result-icon {
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: var(--color-control);
    font-size: 24px;
  }
  .result-icon.paid {
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-primary) 15%, transparent);
  }
  h2 {
    font-size: 30px;
    letter-spacing: -0.04em;
    margin: 0;
  }
  p {
    margin: 0;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  li,
  .amount {
    display: flex;
    justify-content: space-between;
    gap: 20px;
    padding: 14px 0;
    border-bottom: 1px solid var(--color-border);
  }
  li {
    font-size: 13px;
  }
  li span {
    overflow-wrap: anywhere;
    min-width: 0;
  }
  li strong {
    flex-shrink: 0;
  }
  .amount {
    align-items: center;
    border: 0;
  }
  .amount strong {
    font-size: 28px;
    white-space: nowrap;
  }
  @media (max-width: 600px) {
    h2 {
      font-size: 25px;
    }
  }
</style>
