<script lang="ts">
  import LegalNotice from '../../components/LegalNotice.svelte';
  import { money } from '../../lib/format';
  import { apiFetch } from '../../lib/http';
  import { onMount } from 'svelte';
  import { loadStripe } from '@stripe/stripe-js/pure';
  import type { Stripe, StripeElements, StripePaymentElement } from '@stripe/stripe-js';
  import { getStripeConfig, verifyCheckout } from '../../lib/api';
  import type { StripeStatus } from '../../lib/types';
  let {
    intentId,
    amountGrosze,
    onstatus,
  }: {
    intentId: string;
    amountGrosze: number;
    onstatus: (status: StripeStatus) => void;
  } = $props();
  let host: HTMLDivElement;
  let ready = $state(false),
    busy = $state(false),
    error = $state('');
  let stripe: Stripe | null = null,
    elements: StripeElements | undefined,
    payment: StripePaymentElement | undefined;
  onMount(() => {
    let disposed = false;
    async function init() {
      try {
        const [config, response] = await Promise.all([
          getStripeConfig(),
          apiFetch('/api/checkout/intent/' + encodeURIComponent(intentId) + '/resume'),
        ]);
        const saved = await response.json();
        if (!response.ok) throw new Error(saved.message ?? 'Nie udało się wznowić płatności.');
        if (saved.payment_intent_id !== intentId || saved.total_grosze !== amountGrosze)
          throw new Error('Dane płatności różnią się od zamówienia.');
        if (!config.publishable_key) throw new Error('Stripe jest chwilowo niedostępny.');
        stripe = await loadStripe(config.publishable_key, {
          locale: 'pl',
          developerTools: { assistant: { enabled: false } },
        });
        if (disposed) return;
        if (!stripe) throw new Error('Nie udało się wczytać Stripe. Odśwież stronę.');
        const theme = getComputedStyle(document.documentElement);
        elements = stripe.elements({
          clientSecret: saved.client_secret,
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
        payment = elements.create('payment');
        payment.on('ready', () => {
          if (!disposed) ready = true;
        });
        payment.mount(host);
      } catch (cause) {
        if (!disposed)
          error = cause instanceof Error ? cause.message : 'Nie udało się wczytać płatności.';
      }
    }
    void init();
    return () => {
      disposed = true;
      payment?.destroy();
    };
  });
  async function confirm() {
    if (!stripe || !elements || busy || !ready) return;
    busy = true;
    error = '';
    try {
      const result = await stripe.confirmPayment({
        elements,
        confirmParams: {
          return_url:
            window.location.origin + '/zamowienie?payment_intent=' + encodeURIComponent(intentId),
        },
        redirect: 'if_required',
      });
      if (result.error)
        throw new Error(result.error.message ?? 'Nie udało się potwierdzić płatności.');
      const status = await verifyCheckout(intentId);
      onstatus(status);
      if (status.status !== 'paid')
        error = 'Stripe jeszcze przetwarza płatność. Odśwież status za chwilę.';
    } catch (cause) {
      error = cause instanceof Error ? cause.message : 'Nie udało się potwierdzić płatności.';
    } finally {
      busy = false;
    }
  }
</script>

<section class="retry-payment stack" aria-label="Dokończ płatność Stripe">
  <div bind:this={host}></div>
  {#if !ready && !error}<p class="muted" role="status">
      Wczytujemy bezpieczny formularz Stripe…
    </p>{/if}
  {#if error}<p role="alert" class="error">{error}</p>{/if}
  <LegalNotice />
  <button class="button primary" disabled={!ready || busy} onclick={confirm}
    >{busy ? 'Potwierdzamy…' : `Zamawiam i płacę ${money(amountGrosze / 100)} →`}</button
  >
</section>

<style>
  .retry-payment {
    padding-top: 20px;
    border-top: 1px solid var(--color-border);
  }
  .error {
    color: var(--color-danger);
    font-size: 13px;
  }
</style>
