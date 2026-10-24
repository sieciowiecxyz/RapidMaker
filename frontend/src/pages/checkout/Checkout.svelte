<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import * as api from '../../lib/api';
  import ParcelPlan from '../../components/ParcelPlan.svelte';
  import PaymentReturn from './PaymentReturn.svelte';
  import CheckoutSteps from '../../components/CheckoutSteps.svelte';
  import { money, printConfiguration, parcels } from '../../lib/format';
  import type {
    Cart,
    Material,
    Contact,
    DeliveryOption,
    DeliveryMethod,
    Order,
    PickupPoint,
    StripeStatus,
  } from '../../lib/types';
  const routeParams = new URLSearchParams(location.search);
  const paymentReference = routeParams.get('stripe_session') ?? routeParams.get('payment_intent');
  const returningPayment = Boolean(paymentReference);
  let cart = $state<Cart | null>(null),
    deliveries = $state<DeliveryOption[]>([]);
  let busy = $state(true),
    error = $state(''),
    invoice = $state(false);
  let selectedOffer = $state(''),
    paymentMessage = $state('');
  let materials = $state<Material[]>([]);
  let stage = $state(2);
  let stripeKey = $state('');
  let stripeForm = $state<StripeCheckout>();
  import StripeCheckout from './StripeCheckout.svelte';
  let resumeSession = $state('');
  let order = $state<Order | null>(null);
  let stripePayment = $state<StripeStatus | null>(null);
  let receipt = $state<{ items: { name: string; quantity: number }[]; delivery: string } | null>(
    null,
  );
  let pointSearch = $state(''),
    pointBusy = $state(false),
    pointError = $state('');
  let points = $state<PickupPoint[]>([]);
  let choosingPoint = $state(false);
  let otherPointLocation = $state(false);
  let lastPointPostal = '';
  let pointRequest = 0;
  const pointCache = new Map<string, PickupPoint[]>();
  let draftKey = 'rapidmaker.checkout-contact.v2.guest';
  const receiptKey = 'rapidmaker.last-checkout';
  function saveDraft() {
    sessionStorage.setItem(draftKey, JSON.stringify({ contact, invoice }));
  }
  async function searchPoints(query = pointSearch.trim() || contact.postalCode.trim()) {
    if (query.length < 2) return;
    const request = ++pointRequest;
    pointBusy = true;
    pointError = '';
    points = [];
    try {
      const found = pointCache.get(query) ?? (await api.getPickupPoints(query)).points;
      if (request !== pointRequest) return;
      pointCache.set(query, found);
      points = found;
      if (!points.length)
        pointError = 'Brak punktów w tej okolicy. Wybierz inne miejsce lub kuriera.';
    } catch (e) {
      if (request === pointRequest) pointError = (e as Error).message;
    } finally {
      if (request === pointRequest) pointBusy = false;
    }
  }
  $effect(() => {
    const postal = contact.postalCode.trim();
    const locker = cart?.delivery === 'locker';
    untrack(() => {
      if (lastPointPostal && postal !== lastPointPostal) {
        contact.pickupPoint = '';
        points = [];
        choosingPoint = true;
        otherPointLocation = false;
        ++pointRequest;
        pointBusy = false;
        saveDraft();
      }
      lastPointPostal = postal;
    });
    if (!locker || !/^\d{2}-\d{3}$/.test(postal)) return;
    const timer = setTimeout(() => {
      pointSearch = postal;
      void searchPoints(postal);
    }, 350);
    return () => clearTimeout(timer);
  });
  function choosePoint(point: PickupPoint) {
    contact.pickupPoint = point.code;
    choosingPoint = false;
    saveDraft();
    void delivery('locker');
  }

  let contactAtQuote = $state('');
  let quoteBusy = $state(false),
    quoteError = $state('');
  let lastQuoteAttempt = '';
  let quoteController: AbortController | undefined;
  $effect(() => {
    const snapshot = quoteSnapshot();
    if (snapshot !== lastQuoteAttempt) quoteError = '';
    if (
      stage !== 2 ||
      busy ||
      !cart?.models.length ||
      quoteFresh ||
      Object.keys(validateContact()).length ||
      snapshot === lastQuoteAttempt
    )
      return;
    const timer = setTimeout(() => void updateDeliveryQuote(), 600);
    return () => {
      clearTimeout(timer);
      quoteController?.abort();
      lastQuoteAttempt = '';
      quoteBusy = false;
    };
  });
  async function updateDeliveryQuote() {
    const snapshot = quoteSnapshot();
    quoteController?.abort();
    const controller = (quoteController = new AbortController());
    lastQuoteAttempt = snapshot;
    quoteBusy = true;
    quoteError = '';
    try {
      await refreshQuote(controller.signal);
    } catch (e) {
      if (!controller.signal.aborted && snapshot === quoteSnapshot())
        quoteError = (e as Error).message;
    } finally {
      if (controller === quoteController) quoteBusy = false;
    }
  }
  const normalizedContact = () => ({
    ...contact,
    company: invoice ? contact.company : '',
    nip: invoice ? contact.nip : '',
  });
  const quoteSnapshot = () =>
    JSON.stringify({ contact: normalizedContact(), delivery: cart?.delivery });
  const quoteFresh = $derived(contactAtQuote === quoteSnapshot());
  let contact = $state<Contact>({
    name: '',
    email: '',
    phone: '',
    street: '',
    postalCode: '',
    city: '',
    company: '',
    nip: '',
    pickupPoint: '',
  });
  let validationAttempted = $state(false);
  const missingGeometry = $derived(
    cart?.models.some((model) => {
      const dimensions = model.analysis?.dimensions_mm;
      const weight = model.analysis?.weight_g;
      return (
        !dimensions ||
        dimensions.some((value) => !Number.isFinite(value) || value <= 0) ||
        weight == null ||
        !Number.isFinite(weight) ||
        weight <= 0
      );
    }) ?? false,
  );
  const fieldErrors: Partial<Record<keyof Contact, string>> = $derived({
    ...(quoteFresh ? cart?.quote?.checkout?.field_errors : {}),
    ...(validationAttempted ? validateContact() : {}),
  });
  function validateContact(): Partial<Record<keyof Contact, string>> {
    const errors: Partial<Record<keyof Contact, string>> = {};
    if (!contact.name.trim()) errors.name = 'Podaj imię i nazwisko.';
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(contact.email.trim()))
      errors.email = 'Podaj poprawny adres email, np. jan@example.com.';
    if (!/^\+?\d{9,15}$/.test(contact.phone.replace(/[\s()-]/g, '')))
      errors.phone = 'Podaj numer telefonu: 9 cyfr lub numer z prefiksem kraju.';
    if (cart?.delivery === 'courier') {
      if (!/^\d{2}-\d{3}$/.test(contact.postalCode.trim()))
        errors.postalCode = 'Podaj kod pocztowy w formacie 00-001.';
      if (!contact.city.trim()) errors.city = 'Podaj miejscowość.';
      if (!contact.street.trim()) errors.street = 'Podaj ulicę i numer budynku.';
      else if (!/\d/.test(contact.street))
        errors.street = 'Dopisz numer budynku; jeśli nie ma ulicy, wpisz miejscowość i numer.';
    }
    if (cart?.delivery === 'locker' && !/^\d{2}-\d{3}$/.test(contact.postalCode.trim()))
      errors.postalCode = 'Podaj kod pocztowy w danych odbiorcy.';
    if (cart?.delivery === 'locker' && !contact.pickupPoint)
      errors.pickupPoint = 'Wybierz Paczkomat z listy punktów.';
    if (invoice) {
      if (!contact.company?.trim()) errors.company = 'Podaj nazwę firmy.';
      if (!/^\d{10}$/.test(contact.nip ?? '')) errors.nip = 'Podaj NIP składający się z 10 cyfr.';
    }
    return errors;
  }
  async function checkContact() {
    validationAttempted = true;
    if (!Object.keys(validateContact()).length) return true;
    error = 'Popraw oznaczone pola, aby sprawdzić dostawę.';
    await tick();
    document.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus();
    return false;
  }
  onMount(() => {
    load();
  });
  async function load() {
    busy = true;
    error = '';
    try {
      if (paymentReference) {
        stripePayment = await api.verifyCheckout(paymentReference);
        order = api.getCheckoutOrder(paymentReference);
        const saved = JSON.parse(sessionStorage.getItem(receiptKey) ?? 'null');
        if (saved?.sessionId === paymentReference) receipt = saved;
        return;
      }
      stripeKey = (await api.getStripeConfig()).publishable_key ?? '';
      const savedCart = await api.getCart();
      deliveries = await api.getDeliveryOptions();
      materials = await api.getMaterials();
      const user = await api.getSession();
      draftKey = 'rapidmaker.checkout-contact.v2.' + (user?.id ?? 'guest');
      if (user && !user.id.startsWith('demo-')) contact = (await api.getProfile()).contact;
      const saved = JSON.parse(sessionStorage.getItem(draftKey) ?? 'null');
      if (saved?.contact && typeof saved.contact.email === 'string') {
        contact = { ...contact, ...saved.contact };
        invoice = !!saved.invoice;
      }
      pointSearch = contact.postalCode || contact.city;
      cart = savedCart;
      if (cart.models.length) await refreshQuote();
      if (routeParams.has('stripe_cancelled')) {
        paymentMessage = 'Płatność anulowana. Twoje modele i dane zostały zachowane.';
        const receipt = JSON.parse(sessionStorage.getItem(receiptKey) ?? 'null');
        if (receipt?.sessionId && api.getCheckoutOrder(receipt.sessionId))
          resumeSession = receipt.sessionId;
      }
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
  async function refreshQuote(signal?: AbortSignal) {
    if (!cart) return;
    saveDraft();
    const snapshot = quoteSnapshot();
    const quote = await api.quoteCart(normalizedContact(), signal);
    if (snapshot !== quoteSnapshot()) return;
    cart = { ...cart, quote };
    contactAtQuote = snapshot;
    selectedOffer = quote.checkout?.offers.some((offer) => offer.id === selectedOffer)
      ? selectedOffer
      : (quote.checkout?.offers[0]?.id ?? '');
    selectOffer(selectedOffer);
  }
  async function delivery(method: DeliveryMethod) {
    quoteError = '';
    busy = true;
    error = '';
    try {
      const quote = cart?.quote ?? null;
      cart = { ...(await api.setDelivery(method, contact.postalCode)), quote };
      contactAtQuote = '';
      lastQuoteAttempt = '';
      selectedOffer = '';
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
  async function review() {
    if (!cart || busy || quoteBusy) return;
    validationAttempted = true;
    if (stripeForm && cart.delivery === 'courier') {
      try {
        await stripeForm.validateAddress();
      } catch (e) {
        error = (e as Error).message;
        return;
      }
    }
    if (!(await checkContact())) return;
    busy = true;
    error = '';
    try {
      if (cart.delivery === 'locker' && !contact.pickupPoint) throw new Error('Wybierz Paczkomat.');
      if (!quoteFresh || Date.now() / 1000 >= (cart.quote?.checkout?.expires_at ?? 0))
        await refreshQuote();
      if (!quoteFresh) throw new Error('Dane zmieniły się podczas wyceny. Sprawdź je ponownie.');
      if (!cart?.quote?.manual && !selectedOffer)
        throw new Error(
          cart?.quote?.reason ?? 'Przewoźnik nie zwrócił oferty. Wybierz inną metodę dostawy.',
        );
      stage = 3;
      window.scrollTo({ top: 0, behavior: 'smooth' });
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
  function editDelivery() {
    stage = 2;
    error = '';
    window.scrollTo({ top: 0, behavior: 'smooth' });
  }
  async function submit() {
    if (!cart?.quote?.checkout || busy) return;
    busy = true;
    error = '';
    try {
      if (!quoteFresh || Date.now() / 1000 >= cart.quote.checkout.expires_at) {
        stage = 2;
        throw new Error('Wycena wygasła lub dane się zmieniły. Przejdź ponownie do podsumowania.');
      }
      if (cart.quote.manual) {
        throw new Error(
          'Ręczna wycena nie jest jeszcze obsługiwana online. Skontaktuj się z zespołem.',
        );
      }
      if (!selectedOffer) {
        throw new Error('Dane lub wycena zmieniły się. Przejdź ponownie do podsumowania.');
      }
      if (stripeForm && cart.delivery === 'courier') {
        await stripeForm.preparePayment();
        const intent = await api.startOrderIntent(cart.quote, normalizedContact(), selectedOffer);
        const saved = {
          sessionId: intent.payment_intent_id,
          orderId: intent.order.id,
          items: cart.models.map((m) => ({ name: m.name, quantity: m.options.quantity })),
          delivery:
            cart.quote.checkout.offers.find((o) => o.id === selectedOffer)?.name ?? 'Kurier',
        };
        sessionStorage.setItem(receiptKey, JSON.stringify(saved));
        // Keep the receipt address after an inline payment, including a later reload.
        history.replaceState(
          {},
          '',
          '/zamowienie?payment_intent=' + encodeURIComponent(intent.payment_intent_id),
        );
        await stripeForm.confirm(intent.client_secret, contact.email, intent.payment_intent_id);
        stripePayment = await api.verifyCheckout(intent.payment_intent_id);
        order = api.getCheckoutOrder(intent.payment_intent_id);
        receipt = saved;
        stage = 4;
        return;
      }
      const result = await api.startOrderCheckout(cart.quote, normalizedContact(), selectedOffer);
      const url = new URL(result.url);
      if (url.protocol !== 'https:' || url.hostname !== 'checkout.stripe.com')
        throw new Error('Nieprawidłowy adres płatności.');
      saveDraft();
      sessionStorage.setItem(
        receiptKey,
        JSON.stringify({
          sessionId: result.session_id,
          orderId: result.order.id,
          items: cart.models.map((m) => ({ name: m.name, quantity: m.options.quantity })),
          delivery:
            cart.delivery === 'pickup'
              ? 'Odbiór osobisty'
              : (cart.quote.checkout.offers.find((o) => o.id === selectedOffer)?.name ?? 'Kurier') +
                (cart.delivery === 'locker' ? ' · ' + contact.pickupPoint : ''),
        }),
      );
      stage = 4;
      location.assign(url.href);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
  function selectOffer(id: string) {
    if (!cart?.quote?.checkout) return;
    selectedOffer = id;
    const offer = cart.quote.checkout.offers.find((item) => item.id === id);
    if (offer)
      cart = {
        ...cart,
        quote: {
          ...cart.quote,
          delivery: offer.price_grosze / 100,
          total:
            cart.quote.subtotal +
            cart.quote.checkout.packaging_grosze / 100 +
            offer.price_grosze / 100,
        },
      };
  }
</script>

<section class="page purchase-page">
  <CheckoutSteps
    current={returningPayment ? 4 : stage}
    onStep={returningPayment || order
      ? undefined
      : (step) => {
          if (step === 2) editDelivery();
          else {
            stage = step;
            error = '';
            window.scrollTo({ top: 0, behavior: 'smooth' });
          }
        }}
  />
  <div class="page-heading">
    <span class="badge"
      >{returningPayment || stage === 4
        ? '04 / Płatność'
        : stage === 3
          ? '03 / Podsumowanie'
          : '02 / Wysyłka'}</span
    >
    <h1>
      {returningPayment
        ? 'Status płatności.'
        : stage === 4
          ? 'Dokończ płatność.'
          : stage === 3
            ? 'Sprawdź zamówienie.'
            : 'Dokąd wysłać wydruk?'}
    </h1>
    <p class="muted">
      {returningPayment
        ? 'Potwierdzenie i szczegóły Twojej płatności.'
        : stage === 4
          ? 'Kwota jest ustalona. Potwierdź płatność w bezpiecznym formularzu Stripe.'
          : stage === 3
            ? 'Modele, dostawa i pełna cena — wszystko w jednym miejscu.'
            : 'Podaj dane raz. Na ich podstawie dobierzemy dostawę i pobliskie punkty odbioru.'}
    </p>
  </div>
  {#if error && !returningPayment}<p class="alert error" role="alert">
      {error}{#if !cart}<button class="button ghost" onclick={load}>Spróbuj ponownie</button>{/if}
    </p>{/if}
  {#if returningPayment && error}<section class="panel stack payment-error">
      <h2>Nie udało się sprawdzić płatności</h2>
      <p class="muted">
        Nie znamy jeszcze jej wyniku. Sprawdź ponownie lub otwórz swoje zamówienia.
      </p>
      <p class="alert error" role="alert">{error}</p>
      <div class="toolbar">
        <button class="button primary" disabled={busy} onclick={load}>Sprawdź ponownie</button>
        <a class="button secondary" href="/konto/zamowienia">Moje zamówienia</a>
      </div>
    </section>
  {:else if stripePayment}<PaymentReturn payment={stripePayment} {receipt} {order} />
  {:else if busy && !cart}<p class="panel" aria-live="polite">
      {returningPayment ? 'Sprawdzamy wynik płatności…' : 'Przygotowujemy podsumowanie…'}
    </p>
  {:else if order}<section class="panel stack confirmation">
      <h2>Prośba o wycenę zapisana</h2>
      <strong>{order.number}</strong>
      <p>{order.contact.email}</p>
      <p class="muted">Wymagania czekają na ocenę. Płatność nie została pobrana.</p>
      <a
        class="button primary"
        href={'/logowanie?register=1&email=' + encodeURIComponent(order.contact.email)}
        >Utwórz konto przez email →</a
      ><a href="/konto/zamowienia">Moje zamówienia</a>
    </section>
  {:else if error && !cart}
    <!-- Błąd odczytu jest pokazany powyżej; nie oznacza pustego koszyka. -->
  {:else if !cart?.models.length}<div class="panel empty-state">
      <h2>Najpierw Twój model</h2>
      <p class="muted">Koszyk czeka na pierwszy projekt.</p>
      <a class="button primary" href="/wycena?restore=cart">Przejdź do wyceny →</a>
    </div>
  {:else}
    {#if paymentMessage}<div class="alert stack" role="status">
        <span>{paymentMessage}</span>
        {#if resumeSession}<a
            href={'/zamowienie?stripe_session=' + encodeURIComponent(resumeSession)}
            >Wróć do rozpoczętej płatności →</a
          >{/if}
      </div>{/if}
    <form
      class="checkout-layout work-surface"
      novalidate
      oninput={saveDraft}
      onchange={saveDraft}
      onsubmit={(e) => {
        e.preventDefault();
        if (stage === 2) review();
        else if (
          stage === 3 &&
          !cart?.quote?.manual &&
          stripeForm &&
          cart?.delivery === 'courier'
        ) {
          stage = 4;
          error = '';
          window.scrollTo({ top: 0, behavior: 'smooth' });
        } else submit();
      }}
    >
      <div class="stack">
        <section
          class="checkout-section stack"
          hidden={stage === 3 || (stage === 4 && cart.delivery !== 'courier')}
        >
          <div class="section-title" hidden={stage !== 2}>
            <span>01</span>
            <h2>Dane odbiorcy</h2>
          </div>
          <div class="recipient-fields">
            <div class="stack recipient-contact" hidden={stage !== 2}>
              <div class="contact-grid">
                {#if !stripeKey}<label class="contact-name"
                    >Imię i nazwisko<input
                      required
                      autocomplete="shipping name"
                      id="checkout-name"
                      aria-invalid={!!fieldErrors.name}
                      aria-describedby={fieldErrors.name ? 'error-name' : undefined}
                      bind:value={contact.name}
                    />{#if fieldErrors.name}<small class="field-error" id="error-name"
                        >{fieldErrors.name}</small
                      >{/if}</label
                  >{/if}<label
                  >Email<input
                    required
                    type="email"
                    autocomplete="shipping email"
                    id="checkout-email"
                    aria-invalid={!!fieldErrors.email}
                    aria-describedby={fieldErrors.email ? 'error-email' : undefined}
                    bind:value={contact.email}
                  />{#if fieldErrors.email}<small class="field-error" id="error-email"
                      >{fieldErrors.email}</small
                    >{/if}</label
                >
                <label
                  >Telefon<input
                    required
                    type="tel"
                    autocomplete="shipping tel"
                    id="checkout-phone"
                    aria-invalid={!!fieldErrors.phone}
                    aria-describedby={fieldErrors.phone ? 'error-phone' : undefined}
                    bind:value={contact.phone}
                  />{#if fieldErrors.phone}<small class="field-error" id="error-phone"
                      >{fieldErrors.phone}</small
                    >{/if}</label
                >
              </div>
            </div>
            {#if stripeKey}<div class="stack">
                <StripeCheckout
                  bind:this={stripeForm}
                  publishableKey={stripeKey}
                  amount={Math.round((cart.quote?.total ?? 1) * 100)}
                  {contact}
                  paymentVisible={stage === 4}
                  onchange={(values) => {
                    contact = { ...contact, ...values };
                    saveDraft();
                  }}
                />
                {#if stage === 2}{#each ['name', 'street', 'postalCode', 'city'] as field}{#if fieldErrors[field as keyof Contact]}<p
                        class="field-error"
                        role="alert"
                      >
                        {fieldErrors[field as keyof Contact]}
                      </p>{/if}{/each}{/if}
              </div>{:else}<div class="stack" hidden={stage !== 2}>
                <div class="form-grid">
                  <label
                    >Kod pocztowy<input
                      required
                      pattern={'[0-9]{2}-[0-9]{3}'}
                      placeholder="00-001"
                      autocomplete="shipping postal-code"
                      id="checkout-postalCode"
                      aria-invalid={!!fieldErrors.postalCode}
                      aria-describedby={fieldErrors.postalCode ? 'error-postalCode' : undefined}
                      bind:value={contact.postalCode}
                    />{#if fieldErrors.postalCode}<small class="field-error" id="error-postalCode"
                        >{fieldErrors.postalCode}</small
                      >{/if}</label
                  ><label
                    >Miejscowość<input
                      required
                      autocomplete="shipping address-level2"
                      id="checkout-city"
                      aria-invalid={!!fieldErrors.city}
                      aria-describedby={fieldErrors.city ? 'error-city' : undefined}
                      bind:value={contact.city}
                    />{#if fieldErrors.city}<small class="field-error" id="error-city"
                        >{fieldErrors.city}</small
                      >{/if}</label
                  >
                </div>
                <label
                  >Ulica, numer domu i mieszkania<input
                    required
                    autocomplete="shipping street-address"
                    id="checkout-street"
                    aria-invalid={!!fieldErrors.street}
                    aria-describedby={fieldErrors.street ? 'error-street' : undefined}
                    bind:value={contact.street}
                  />{#if fieldErrors.street}<small class="field-error" id="error-street"
                      >{fieldErrors.street}</small
                    >{/if}</label
                >
              </div>{/if}
          </div>
          <div class="stack invoice-fields" hidden={stage !== 2}>
            <label class="checkbox"
              ><input type="checkbox" bind:checked={invoice} />Chcę fakturę na firmę</label
            >{#if invoice}<label
                >Nazwa firmy<input
                  required
                  id="checkout-company"
                  aria-invalid={!!fieldErrors.company}
                  aria-describedby={fieldErrors.company ? 'error-company' : undefined}
                  bind:value={contact.company}
                  autocomplete="organization"
                />{#if fieldErrors.company}<small class="field-error" id="error-company"
                    >{fieldErrors.company}</small
                  >{/if}</label
              ><label
                >NIP<input
                  required
                  pattern={'[0-9]{10}'}
                  maxlength="10"
                  placeholder="10 cyfr"
                  id="checkout-nip"
                  aria-invalid={!!fieldErrors.nip}
                  aria-describedby={fieldErrors.nip ? 'error-nip' : undefined}
                  bind:value={contact.nip}
                />{#if fieldErrors.nip}<small class="field-error" id="error-nip"
                    >{fieldErrors.nip}</small
                  >{/if}</label
              >{/if}
          </div>
        </section>
        <div class="stack" hidden={stage !== 2}>
          <section class="checkout-section stack">
            <div class="section-title">
              <span>02</span>
              <h2>Dostawa</h2>
            </div>
            <div class="delivery-options">
              {#each deliveries as method}<label
                  class:active={cart.delivery === method.id}
                  class="delivery-option"
                  ><input
                    type="radio"
                    name="delivery"
                    value={method.id}
                    checked={cart.delivery === method.id}
                    disabled={busy}
                    onchange={() => delivery(method.id)}
                  /><span><strong>{method.name}</strong><small>{method.description}</small></span><b
                    >{method.id === 'pickup'
                      ? 'Bezpłatnie'
                      : cart.delivery === method.id && quoteFresh && !cart.quote?.deliveryPending
                        ? money(cart.quote?.delivery ?? 0)
                        : cart.delivery === method.id
                          ? '—'
                          : 'Wybierz'}</b
                  ></label
                >{/each}
            </div>
            {#if cart.delivery === 'pickup'}
              <p class="muted">Odbiór po potwierdzeniu gotowości i uzgodnieniu terminu.</p>
            {:else if cart.delivery === 'locker'}
              {#if contact.pickupPoint && !choosingPoint}
                <div class="selected-point-row">
                  <span
                    ><small class="muted">Wybrany Paczkomat</small><strong
                      >{contact.pickupPoint}</strong
                    ></span
                  ><button type="button" class="button ghost" onclick={() => (choosingPoint = true)}
                    >Zmień punkt</button
                  >
                </div>
              {:else}
                <p class="muted">
                  {pointBusy
                    ? 'Szukamy Paczkomatów…'
                    : `Paczkomaty w pobliżu ${contact.postalCode || 'Twojego adresu'}`}
                </p>
                {#if !/^\d{2}-\d{3}$/.test(contact.postalCode.trim())}
                  <p class="muted">Uzupełnij kod pocztowy w danych odbiorcy powyżej.</p>
                {/if}
                <button
                  type="button"
                  class="text-button"
                  onclick={() => {
                    otherPointLocation = !otherPointLocation;
                    pointSearch = '';
                  }}>Szukaj w innym miejscu</button
                >
                {#if otherPointLocation}
                  <div class="alternate-point-search">
                    <label
                      >Szukaj punktu<input
                        placeholder="Miasto, ulica lub nazwa Paczkomatu"
                        bind:value={pointSearch}
                        onkeydown={(event) => {
                          if (event.key === 'Enter') {
                            event.preventDefault();
                            void searchPoints();
                          }
                        }}
                      /></label
                    >
                    <button
                      type="button"
                      class="button secondary"
                      disabled={pointBusy || pointSearch.trim().length < 2}
                      onclick={() => searchPoints()}>Szukaj</button
                    >
                  </div>
                {/if}
                {#if pointError}<p class="muted" role="status">{pointError}</p>{/if}
                {#if contact.pickupPoint}<p class="selected-point">
                    Wybrano: <strong>{contact.pickupPoint}</strong>
                  </p>{/if}
                <ul class="point-list">
                  {#each points as point}<li>
                      <button
                        type="button"
                        class:chosen={contact.pickupPoint === point.code}
                        disabled={busy}
                        onclick={() => choosePoint(point)}
                        ><span
                          ><strong>{point.code}</strong><small
                            >{point.address.street} · {point.address.postcode}
                            {point.address.city}</small
                          ></span
                        ><span aria-hidden="true"
                          >{contact.pickupPoint === point.code ? '✓' : '+'}</span
                        ></button
                      >
                    </li>{/each}
                </ul>
              {/if}
              {#if fieldErrors.pickupPoint}<small class="field-error" id="error-pickupPoint"
                  >{fieldErrors.pickupPoint}</small
                >{/if}
              <small class="muted"
                >Paczkomat jest obecnie dostępny dla jednej paczki. Większe partie wyślemy kurierem.</small
              >
            {/if}
            {#if cart.delivery !== 'pickup'}
              {#if missingGeometry}
                <div class="alert stack" role="status">
                  <span
                    >Analiza modelu jest wymagana do wyceny dostawy. Wymiary, masę i liczbę paczek
                    ustalimy na podstawie modelu.</span
                  >
                  <a href="/wycena?restore=cart">Wróć do konfiguracji modelu →</a>
                  <small>Możesz też przejść dalej i poprosić zespół o ręczną wycenę.</small>
                </div>
              {/if}
              {#if quoteBusy}<p class="muted" role="status">Pobieramy dostępne dostawy…</p>
              {:else if quoteError}<div class="alert" role="alert">
                  <span>{quoteError}</span><button
                    type="button"
                    class="text-button"
                    onclick={updateDeliveryQuote}>Spróbuj ponownie</button
                  >
                </div>
              {:else if quoteFresh && cart.quote?.checkout?.offers.length}
                {#each cart.quote.checkout.offers as offer, index}
                  <label class="delivery-option" class:active={selectedOffer === offer.id}
                    ><input
                      type="radio"
                      name="carrier"
                      checked={selectedOffer === offer.id}
                      onchange={() => selectOffer(offer.id)}
                    /><span
                      >{offer.name}{#if index === 0}<small
                          >Najtańsza dostępna oferta dla tych paczek</small
                        >{/if}</span
                    ><b>{money(offer.price_grosze / 100)}</b></label
                  >
                {/each}
              {:else if quoteFresh && !Object.keys(validateContact()).length}<div
                  class="alert"
                  role="status"
                >
                  <span
                    >{cart.quote?.reason ??
                      'Brak dostawy dla tych paczek. Wybierz inną metodę lub poproś o wycenę.'}</span
                  >
                  {#if !Object.keys(cart.quote?.checkout?.field_errors ?? {}).length}<button
                      type="button"
                      class="text-button"
                      onclick={updateDeliveryQuote}>Spróbuj ponownie</button
                    >{/if}
                </div>{:else}<p class="muted">
                  {cart.delivery === 'locker' && !contact.pickupPoint
                    ? 'Wybierz Paczkomat, aby zobaczyć cenę.'
                    : 'Uzupełnij dane powyżej — ceny dostawy pojawią się automatycznie.'}
                </p>{/if}
            {/if}
          </section>
        </div>
        {#if stage === 3}
          <section class="checkout-section stack">
            <div class="review-heading">
              <h2>Wydruki</h2>
              <a href="/wycena?restore=cart">Zmień konfigurację</a>
            </div>
            {#each cart.models as model}
              <details class="review-model">
                <summary
                  ><span
                    ><strong>{model.name}</strong><small
                      >{model.options.quantity} szt. · {materials.find(
                        (material) => material.id === model.options.materialId,
                      )?.name ?? model.options.materialId} · {model.options.color}<span
                        class="details-hint"
                      >
                        · Parametry ↓</span
                      ></small
                    ></span
                  ><b
                    >{money(
                      cart.quote?.lines.find((line) => line.modelId === model.id)?.total ?? 0,
                    )}</b
                  ></summary
                >
                <div class="stack">
                  {#each printConfiguration(model) as setting}<small class="muted">{setting}</small
                    >{/each}{#if model.notes}<small>Uwagi: {model.notes}</small>{/if}
                </div>
              </details>
            {/each}
          </section>
          <section class="checkout-section stack">
            <div class="review-heading">
              <h2>Dostawa i kontakt</h2>
              <button class="text-button" type="button" onclick={editDelivery}>Zmień</button>
            </div>
            <strong
              >{cart.delivery === 'pickup'
                ? 'Odbiór osobisty'
                : (cart.quote?.checkout?.offers.find((offer) => offer.id === selectedOffer)?.name ??
                  'Dostawa do potwierdzenia')}</strong
            >
            {#if cart.delivery === 'locker'}<p>
                Paczkomat {contact.pickupPoint}
              </p>{:else if cart.delivery === 'courier'}<p>
                {contact.street}<br />{contact.postalCode}
                {contact.city}
              </p>{:else}<p class="muted">
                Termin odbioru ustalimy po przygotowaniu wydruków.
              </p>{/if}
            <p>{contact.name}<br />{contact.email}<br />{contact.phone}</p>
            {#if invoice}<p>Faktura: {contact.company} · NIP {contact.nip}</p>{/if}
            {#if cart.delivery !== 'pickup' && cart.quote?.checkout}<details
                class="packing-details"
              >
                <summary
                  >{cart.quote.checkout.packing.parcels.length
                    ? parcels(cart.quote.checkout.packing.parcels.length) + ' · '
                    : ''}Szczegóły pakowania</summary
                >
                <ParcelPlan
                  packing={cart.quote.checkout.packing}
                  models={cart.models}
                  delivery={cart.delivery}
                />
              </details>{/if}
          </section>
        {:else if stage === 4 && !(stripeKey && cart.delivery === 'courier')}<p
            class="muted"
            role="status"
          >
            Otwieramy bezpieczną stronę Stripe…
          </p>{/if}
      </div>
      <aside class="stack summary">
        <span class="eyebrow">Podsumowanie</span>
        <h2>Twoje zamówienie</h2>
        {#each cart.quote?.lines ?? [] as line}<div class="line">
            <span>{line.name}<small>{line.quantity} szt. × {money(line.unitPrice)}</small></span
            ><strong>{money(line.total)}</strong>
          </div>{/each}
        {#if cart.quote}
          <div class="line">
            <span>Przygotowanie</span><strong
              >{money((cart.quote.checkout?.setup_grosze ?? 0) / 100)}</strong
            >
          </div>
          <div class="line">
            <span>Opakowania</span><strong
              >{money((cart.quote.checkout?.packaging_grosze ?? 0) / 100)}</strong
            >
          </div>
          <div class="line">
            <span>Dostawa{cart.quote.packages ? ' · ' + parcels(cart.quote.packages) : ''}</span
            ><strong
              >{cart.quote.deliveryPending || !quoteFresh
                ? 'Do potwierdzenia'
                : money(cart.quote.delivery)}</strong
            >
          </div>
          <div class="order-actions">
            <div class="total">
              <span
                >{cart.quote.deliveryPending || !quoteFresh
                  ? 'Suma bez dostawy'
                  : 'Razem brutto'}</span
              ><strong
                >{money(
                  quoteFresh
                    ? cart.quote.total
                    : cart.quote.subtotal + (cart.quote.checkout?.packaging_grosze ?? 0) / 100,
                )}</strong
              >
            </div>
            <button
              type="submit"
              class="button primary"
              disabled={busy ||
                quoteBusy ||
                (stage === 4 && !(stripeForm && cart.delivery === 'courier')) ||
                (stage === 3 && cart.quote.manual) ||
                (stage === 3 &&
                  !cart.quote.manual &&
                  (!quoteFresh || !selectedOffer || !cart.quote.checkout?.stripe_configured))}
            >
              {busy || quoteBusy
                ? 'Sprawdzamy…'
                : stage === 2
                  ? 'Podsumowanie →'
                  : cart.quote.manual
                    ? 'Skontaktuj się w sprawie wyceny'
                    : stage === 4 && stripeForm && cart.delivery === 'courier'
                      ? `Zapłać ${money(cart.quote.total)} →`
                      : 'Przejdź do płatności →'}
            </button>
          </div>
          {#if stage !== 2 && cart.quote.reason}<p class="muted">{cart.quote.reason}</p>{/if}
          <small class="muted">Płatność testowa · ceny wydruku wymagają kalibracji.</small>
        {/if}
        {#if stage === 4}<button
            type="button"
            class="text-button"
            onclick={() => {
              stage = 3;
              error = '';
            }}>← Wróć do podsumowania</button
          >{:else if stage === 3}<button type="button" class="text-button" onclick={editDelivery}
            >← Wróć do dostawy</button
          >{:else}<a href="/wycena?restore=cart">← Wróć do konfiguracji druku</a>{/if}
      </aside>
    </form>
  {/if}
</section>

<style>
  [hidden] {
    display: none !important;
  }
  .alternate-point-search {
    display: flex;
    align-items: end;
    gap: 12px;
  }
  .alternate-point-search label {
    flex: 1;
    min-width: 0;
  }
  .recipient-fields {
    display: grid;
    gap: 18px;
  }
  .recipient-contact label {
    line-height: 18px;
  }
  .payment-error {
    max-width: 680px;
  }
  .payment-error p {
    margin: 0;
  }
  .recipient-contact input {
    height: 44px;
    line-height: 18px;
  }
  .contact-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 18px;
  }
  .contact-name {
    grid-column: 1 / -1;
  }
  @media (max-width: 600px) {
    .contact-grid {
      grid-template-columns: 1fr;
    }
  }
  .checkout-layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 330px;
    gap: 80px;
    align-items: start;
  }
  .checkout-layout > .stack {
    gap: 0;
  }
  .checkout-section {
    padding: 0 0 36px;
    gap: 20px;
  }
  .section-title {
    display: flex;
    align-items: baseline;
    gap: 18px;
    margin-bottom: 4px;
  }
  .section-title > span {
    font-size: 11px;
    color: var(--color-muted);
    font-variant-numeric: tabular-nums;
  }
  .section-title h2 {
    margin: 0;
    font-size: 21px;
    letter-spacing: -0.025em;
  }
  .checkout-section > p,
  .checkout-section > small {
    font-size: 12px;
    line-height: 1.6;
    margin: 0;
  }
  .selected-point-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 0;
  }
  .selected-point-row strong,
  .selected-point-row small {
    display: block;
  }
  .selected-point-row strong {
    margin-top: 5px;
  }
  .packing-details > summary {
    cursor: pointer;
    color: var(--color-muted);
    font-size: 12px;
  }
  .packing-details[open] > summary {
    margin-bottom: 18px;
  }
  .point-list {
    list-style: none;
    padding: 0;
    margin: 0;
    max-height: 300px;
    overflow: auto;
  }
  .point-list button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    text-align: left;
    background: transparent;
    color: inherit;
    border: 0;
    border-bottom: 1px solid var(--color-border);
    width: 100%;
    padding: 14px 8px;
    cursor: pointer;
  }
  .point-list button.chosen {
    color: var(--color-accent);
  }
  .point-list small {
    display: block;
    color: var(--color-muted);
    margin-top: 5px;
  }
  .selected-point {
    margin: 0;
    font-size: 13px;
  }
  .summary {
    position: sticky;
    top: 28px;
    border-left: 1px solid var(--color-border);
    padding: 6px 0 6px 30px;
    gap: 18px;
  }
  .eyebrow {
    color: var(--color-muted);
    font-size: 11px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  .summary h2 {
    font-size: 23px;
    margin: 0;
    letter-spacing: -0.03em;
  }
  .summary > p,
  .summary > small,
  .summary > a {
    font-size: 12px;
    line-height: 1.6;
    margin: 0;
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    padding: 8px 0;
    font-size: 13px;
  }
  .line strong {
    font-weight: 500;
    text-align: right;
    flex-shrink: 0;
  }
  .line span {
    overflow-wrap: anywhere;
  }
  .line small {
    display: block;
    margin-top: 6px;
    color: var(--color-muted);
    font-size: 11px;
  }
  .total {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding: 6px 0;
  }
  .total span {
    font-size: 13px;
  }
  .total strong {
    font-size: 30px;
    letter-spacing: -0.04em;
    white-space: nowrap;
  }
  .delivery-options {
    display: grid;
    gap: 0;
  }
  .delivery-option {
    display: flex;
    align-items: center;
    gap: 14px;
    border-radius: 8px;
    padding: 17px 12px;
    cursor: pointer;
    background: transparent;
  }
  .delivery-option.active {
    background: var(--color-surface);
  }
  .delivery-option.active strong {
    color: var(--color-accent);
  }
  .delivery-option input,
  .checkbox input {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    accent-color: var(--color-primary);
  }
  .delivery-option span {
    flex: 1;
  }
  .delivery-option strong {
    font-size: 14px;
    font-weight: 500;
  }
  .delivery-option small {
    display: block;
    color: var(--color-muted);
    margin-top: 5px;
    font-size: 11px;
    line-height: 1.5;
  }
  .delivery-option b {
    white-space: nowrap;
    font-size: 12px;
    font-weight: 500;
  }
  label:not(.delivery-option):not(.checkbox) {
    display: grid;
    gap: 8px;
    font-size: 12px;
  }
  .checkbox {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 44px;
    font-size: 13px;
  }
  .review-heading {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 20px;
  }
  .review-heading h2 {
    font-size: 21px;
    margin: 0;
  }
  .review-heading a,
  .text-button {
    font-size: 12px;
  }
  .text-button {
    border: 0;
    padding: 0;
    background: transparent;
    color: var(--color-accent);
    cursor: pointer;
    text-align: left;
  }
  .review-model,
  .packing-details {
    border: 0;
    padding: 0;
  }
  .review-model summary {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 20px;
    cursor: pointer;
  }
  .review-model strong {
    font-size: 15px;
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .review-model small {
    display: block;
    color: var(--color-muted);
    margin-top: 8px;
    font-size: 12px;
  }
  .details-hint {
    margin-inline-start: 0.35em;
    color: var(--color-accent);
    white-space: nowrap;
  }
  .review-model b {
    font-size: 14px;
    font-weight: 500;
    white-space: nowrap;
  }
  .review-model > .stack {
    padding-top: 16px;
    gap: 6px;
  }
  .field-error {
    color: #ffb8a8;
    font-size: 12px;
    line-height: 1.5;
  }
  input[aria-invalid='true'] {
    border-color: #ffb8a8;
  }
  @media (max-width: 1100px) {
    .checkout-layout {
      gap: 36px;
      grid-template-columns: minmax(0, 1fr) 290px;
    }
  }
  @media (max-width: 850px) {
    .checkout-layout {
      grid-template-columns: 1fr;
      gap: 12px;
    }
    .summary {
      position: static;
      padding: 28px 0 0;
      border-left: 0;
      border-top: 1px solid var(--color-border);
    }
    .form-grid {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    }
    .checkout-section {
      padding-block: 24px;
    }
  }
  @media (max-width: 420px) {
    .form-grid {
      grid-template-columns: 1fr;
    }
    .delivery-option {
      gap: 10px;
    }
  }
</style>
