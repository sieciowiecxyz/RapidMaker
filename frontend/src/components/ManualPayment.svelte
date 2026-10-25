<script lang="ts">
  import LegalNotice from './LegalNotice.svelte';
  import { onMount } from 'svelte';
  import { loadStripe } from '@stripe/stripe-js/pure';
  import type { Stripe, StripeElements, StripePaymentElement } from '@stripe/stripe-js';
  import { getStripeConfig } from '../lib/api';
  import type { ManualPayment } from '../lib/manualQuotes';
  import { money } from '../lib/format';
  let {
    payment,
    orderId,
    onchange,
  }: { payment: ManualPayment; orderId: string; onchange: () => void | Promise<void> } = $props();
  let host: HTMLDivElement;
  let ready = $state(false),
    busy = $state(false),
    error = $state('');
  let stripe: Stripe | null = null,
    elements: StripeElements | undefined;
  let active = true;
  onMount(() => {
    let field: StripePaymentElement | undefined;
    void (async () => {
      try {
        if (!payment.client_secret || !payment.payment_intent_id || payment.state !== 'open')
          throw new Error('Płatność nie jest gotowa. Sprawdź ponownie jej status.');
        const config = await getStripeConfig();
        if (!config.publishable_key)
          throw new Error('Formularz płatności jest chwilowo niedostępny.');
        stripe = await loadStripe(config.publishable_key, {
          locale: 'pl',
          developerTools: { assistant: { enabled: false } },
        });
        if (!active) return;
        if (!stripe) throw new Error('Nie udało się wczytać formularza Stripe.');
        const theme = getComputedStyle(document.documentElement);
        elements = stripe.elements({
          clientSecret: payment.client_secret,
          locale: 'pl',
          appearance: {
            theme: document.documentElement.dataset.theme === 'light' ? 'stripe' : 'night',
            variables: {
              colorPrimary: theme.getPropertyValue('--color-primary').trim() || '#b5ee4b',
              colorBackground: theme.getPropertyValue('--color-control').trim() || '#141d15',
              colorText: theme.getPropertyValue('--color-text').trim() || '#edf4e9',
              colorDanger: theme.getPropertyValue('--color-danger').trim() || '#ffc2b8',
              borderRadius:
                theme.getPropertyValue('--control-radius').trim() ||
                theme.getPropertyValue('--radius-sm').trim() ||
                '3px',
            },
          },
        });
        field = elements.create('payment');
        field.on('ready', () => {
          if (active) ready = true;
        });
        field.mount(host);
      } catch (cause) {
        if (active)
          error = cause instanceof Error ? cause.message : 'Nie udało się wczytać płatności.';
      }
    })();
    return () => {
      active = false;
      field?.destroy();
    };
  });
  async function confirm() {
    if (!stripe || !elements || !ready || busy) return;
    busy = true;
    error = '';
    try {
      const result = await stripe.confirmPayment({
        elements,
        confirmParams: {
          return_url:
            location.origin +
            '/konto/zamowienia/' +
            encodeURIComponent(orderId) +
            '?payment=return',
        },
        redirect: 'if_required',
      });
      if (result.error)
        throw new Error(result.error.message ?? 'Nie udało się potwierdzić płatności.');
      if (active) await onchange();
    } catch (cause) {
      if (active)
        error =
          cause instanceof Error ? cause.message : 'Sprawdź status płatności przed ponowieniem.';
    } finally {
      if (active) busy = false;
    }
  }
</script>

<div class="manual-payment">
  <div bind:this={host}></div>
  {#if !ready && !error}<p role="status">Wczytujemy formularz Stripe…</p>{/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <LegalNotice />
  <button class="button primary" disabled={!ready || busy} onclick={confirm}
    >{busy ? 'Potwierdzamy…' : `Zamawiam i płacę ${money(payment.total_grosze / 100)}`}</button
  >
  <small>Dane karty obsługuje Stripe. Zapłatę potwierdza serwer.</small>
</div>

<style>
  .manual-payment {
    display: grid;
    gap: calc(var(--space, 8px) * 2);
  }
  p,
  small {
    color: var(--muted);
    font-size: 12px;
  }
  .error {
    color: var(--danger);
  }
</style>
