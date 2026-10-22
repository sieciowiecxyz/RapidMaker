<script lang="ts">
  import { onMount } from 'svelte';
  import {
    loadStripe,
    type Stripe,
    type StripeElements,
    type StripeAddressElement,
    type StripePaymentElement,
  } from '@stripe/stripe-js';
  import type { Contact } from '../../lib/types';
  let {
    publishableKey,
    amount,
    contact,
    onchange,
    paymentVisible = false,
  }: {
    paymentVisible?: boolean;
    publishableKey: string;
    amount: number;
    contact: Contact;
    onchange: (address: Partial<Contact>) => void;
  } = $props();
  let addressHost: HTMLDivElement;
  let paymentHost: HTMLDivElement;
  let ready = $state(false),
    error = $state('');
  // Stripe owns the iframe fields; no card data enters application state.
  let stripe: Stripe | null,
    elements: StripeElements,
    address: StripeAddressElement,
    payment: StripePaymentElement;
  onMount(() => {
    let disposed = false;
    async function init() {
      try {
        stripe = await loadStripe(publishableKey, {
          locale: 'pl',
          developerTools: { assistant: { enabled: false } },
        });
        if (disposed) return;
        if (!stripe) throw new Error('Nie udało się wczytać Stripe. Odśwież stronę.');
        const theme = getComputedStyle(document.documentElement);
        const token = (name: string) => theme.getPropertyValue(name).trim();
        elements = stripe.elements({
          mode: 'payment',
          amount: Math.max(100, amount),
          currency: 'pln',
          allowedPaymentMethodTypes: ['card'],
          locale: 'pl',
          appearance: {
            theme: 'night',
            variables: {
              colorPrimary: token('--color-primary') || '#b5ee4b',
              colorBackground: token('--color-control') || '#202e1d',
              colorText: token('--color-text') || '#edf4e9',
              colorDanger: token('--color-danger') || '#ffc2b8',
              borderRadius: '3px',
              fontFamily: '"Segoe UI", Arial, sans-serif',
              fontSizeBase: '16px',
              spacingGridRow: '18px',
              spacingGridColumn: '18px',
            },
            rules: {
              '.Label': { fontSize: '12px', lineHeight: '18px', marginBottom: '8px' },
              '.Input': {
                fontSize: '16px',
                lineHeight: '18px',
                padding: '12px',
                border: '1px solid #ffffff18',
                boxShadow: 'none',
              },
            },
          },
        });
        payment = elements.create('payment', { layout: 'tabs', wallets: { link: 'never' } });
        address = elements.create('address', {
          mode: 'shipping',
          allowedCountries: ['PL'],
          autocomplete: { mode: 'automatic' },
          fields: { phone: 'never' },
          defaultValues: {
            name: contact.name,
            address: {
              line1: contact.street,
              city: contact.city,
              postal_code: contact.postalCode,
              country: 'PL',
            },
          },
        });
        address.on('change', (event) => {
          const a = event.value.address;
          onchange({
            name: event.value.name,
            street: [a.line1, a.line2].filter(Boolean).join(', '),
            city: a.city,
            postalCode: a.postal_code,
          });
        });
        payment.mount(paymentHost);
        address.mount(addressHost);
        ready = true;
      } catch (e) {
        error = (e as Error).message;
      }
    }
    void init();
    return () => {
      disposed = true;
      address?.destroy();
      payment?.destroy();
    };
  });
  $effect(() => {
    if (ready) elements.update({ amount: Math.max(100, amount) });
  });
  export async function validateAddress() {
    if (!ready) throw new Error(error || 'Poczekaj na formularz Stripe.');
    const result = await address.getValue();
    if (!result.complete) {
      address.focus();
      throw new Error('Uzupełnij adres w formularzu Stripe.');
    }
  }
  export async function preparePayment() {
    if (!ready) throw new Error('Formularz Stripe nie jest gotowy.');
    const result = await elements.submit();
    if (result.error) throw new Error(result.error.message);
  }
  export async function confirm(clientSecret: string, email: string, intentId: string) {
    const result = await stripe!.confirmPayment({
      elements,
      clientSecret,
      confirmParams: {
        return_url: location.origin + '/zamowienie?payment_intent=' + encodeURIComponent(intentId),
        payment_method_data: { billing_details: { email } },
      },
      redirect: 'if_required',
    });
    if (result.error) throw new Error(result.error.message);
  }
</script>

<section class="stripe-fields">
  <div class="stripe-fields" hidden={paymentVisible}>
    <div bind:this={addressHost}></div>
  </div>
  <div class="stripe-fields" hidden={!paymentVisible}>
    <h3>Płatność kartą · Stripe test</h3>
    <p>Dane karty obsługuje Stripe. Przycisk „Zapłać” zatwierdza podaną kwotę.</p>
    <div bind:this={paymentHost}></div>
  </div>
  {#if !ready && !error}<p role="status">Wczytujemy bezpieczny formularz…</p>{/if}
  {#if error}<p role="alert" class="error">{error}</p>{/if}
</section>

<style>
  [hidden] {
    display: none !important;
  }
  .stripe-fields {
    display: grid;
    gap: 14px;
  }
  h3 {
    font-size: 15px;
    margin: 8px 0 0;
  }
  p {
    font-size: 12px;
    color: var(--color-muted);
    margin: 0;
  }
  p.error {
    color: var(--color-danger);
  }
</style>
