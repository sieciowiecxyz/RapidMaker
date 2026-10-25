<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { getSession } from '../lib/session';
  import { ApiError } from '../lib/http';
  import { getMaterials, getPickupPoints } from '../lib/api';
  import { createManualRequest, manualRequestBody } from '../lib/manualQuotes';
  import type {
    Contact,
    DeliveryMethod,
    Material,
    PickupPoint,
    PrintModel,
    Session,
  } from '../lib/types';
  let {
    models,
    initialContact,
    initialDelivery,
    oncreated,
  }: {
    models: PrintModel[];
    initialContact: Contact;
    initialDelivery: DeliveryMethod;
    oncreated: (id: string) => void | Promise<void>;
  } = $props();
  let user = $state<Session | null>(null),
    materials = $state<Material[]>([]),
    loading = $state(true),
    busy = $state(false);
  let contact = $state<Contact>(untrack(() => ({ ...initialContact }))),
    delivery = $state<DeliveryMethod>(untrack(() => initialDelivery));
  let validationErrors = $state<Record<string, string>>({});
  let notes = $state(''),
    invoice = $state(untrack(() => !!initialContact.company)),
    error = $state('');
  let points = $state<PickupPoint[]>([]),
    pointBusy = $state(false);
  let frozen = $state<(ReturnType<typeof manualRequestBody> & { request_id: string }) | null>(null);
  let active = true,
    pointGeneration = 0;
  const requestId = crypto.randomUUID();
  onMount(() => {
    void (async () => {
      try {
        const [session, catalog] = await Promise.all([getSession(), getMaterials()]);
        if (!active) return;
        user = session;
        materials = catalog;
        if (session) contact.email = session.email;
      } catch (cause) {
        if (active) error = cause instanceof Error ? cause.message : 'Nie udało się wczytać konta.';
      } finally {
        if (active) loading = false;
      }
    })();
    return () => {
      active = false;
      pointGeneration++;
    };
  });
  async function searchPoints() {
    if (pointBusy || !/^\d{2}-\d{3}$/.test(contact.postalCode.trim())) {
      error = 'Podaj poprawny kod pocztowy, aby znaleźć punkty.';
      return;
    }
    const generation = ++pointGeneration;
    pointBusy = true;
    error = '';
    try {
      const result = await getPickupPoints(contact.postalCode.trim());
      if (active && generation === pointGeneration) points = result.points;
    } catch (cause) {
      if (active && generation === pointGeneration)
        error = cause instanceof Error ? cause.message : 'Nie udało się wczytać punktów.';
    } finally {
      if (active && generation === pointGeneration) pointBusy = false;
    }
  }
  async function send() {
    if (busy || !user) return;
    error = '';
    validationErrors = {};
    const retry = !!frozen;
    try {
      if (!frozen) {
        if (
          !contact.name.trim() ||
          !/^\+?[\d\s()-]{9,20}$/.test(contact.phone) ||
          (delivery !== 'pickup' &&
            (!/^\d{2}-\d{3}$/.test(contact.postalCode.trim()) ||
              !contact.city.trim() ||
              !contact.street.trim()))
        )
          throw new Error('Uzupełnij dane odbiorcy, telefon i adres dostawy.');
        if (delivery === 'locker' && !contact.pickupPoint)
          throw new Error('Wybierz punkt odbioru.');
        if (invoice && (!contact.company?.trim() || !/^\d{10}$/.test(contact.nip ?? '')))
          throw new Error('Podaj nazwę firmy i dziesięciocyfrowy NIP.');
        const special = models.flatMap((model) => {
          const a = model.advanced,
            requirements = [
              model.notes,
              model.units !== 'mm' ? `Jednostki STL: ${model.units}` : '',
              a.orientation !== 'auto' ? `Orientacja: ${a.orientation}` : '',
              a.supports !== 'auto' ? `Podpory: ${a.supports}` : '',
              a.finish !== 'standard' ? `Wykończenie: ${a.finish}` : '',
              a.fit !== 'normal' ? `Pasowanie: ${a.fit}` : '',
              a.turnaround !== 'standard' ? `Termin: ${a.turnaround}` : '',
              a.insertCount ? `Inserty: ${a.insertCount} × ${a.insertSize}` : '',
              a.pattern !== 'grid' ? `Wypełnienie: ${a.pattern}` : '',
            ].filter(Boolean);
          return requirements.length ? [`${model.name}: ${requirements.join('; ')}`] : [];
        });
        const requirements = [notes.trim(), ...special].filter(Boolean).join('\n');
        if (new TextEncoder().encode(requirements).length > 4000)
          throw new Error('Uwagi razem z wymaganiami części przekraczają 4000 bajtów. Skróć opis.');
        frozen = {
          ...manualRequestBody(
            models,
            {
              ...contact,
              email: user.email,
              company: invoice ? contact.company : '',
              nip: invoice ? contact.nip : '',
            },
            delivery,
            requirements,
            materials,
          ),
          request_id: requestId,
        };
      }
      busy = true;
      const result = await createManualRequest(frozen);
      if (active) await oncreated(result.order_id);
    } catch (cause) {
      if (active) {
        error =
          cause instanceof Error
            ? cause.message
            : 'Nie znamy wyniku zgłoszenia. Ponów to samo zgłoszenie.';
        const fields = (cause as { fieldErrors?: unknown })?.fieldErrors;
        if (fields && typeof fields === 'object')
          validationErrors = Object.fromEntries(
            Object.entries(fields).filter(
              (entry): entry is [string, string] => typeof entry[1] === 'string',
            ),
          );
        if (!retry && cause instanceof ApiError && [400, 403, 404, 422].includes(cause.status))
          frozen = null;
      }
    } finally {
      if (active) busy = false;
    }
  }
</script>

<section class="manual-request work-surface stack" aria-labelledby="manual-request-heading">
  <h2 id="manual-request-heading">Zapytaj o wycenę</h2>
  <p class="muted">
    Zespół sprawdzi {models.length} części i uzgodni zakres, cenę oraz dostawę. Wysłanie zapytania nie
    pobiera płatności.
  </p>
  {#if error}<p class="alert error" role="alert">{error}</p>{/if}
  {#if Object.keys(validationErrors).length}<ul class="field-errors">
      {#each Object.entries(validationErrors) as [field, message]}<li>
          {(
            {
              name: 'Imię i nazwisko',
              email: 'Email',
              phone: 'Telefon',
              street: 'Ulica i numer',
              postal_code: 'Kod pocztowy',
              city: 'Miejscowość',
              'delivery.point_id': 'Punkt odbioru',
              'invoice.nip': 'NIP',
              'invoice.company': 'Firma',
            } as Record<string, string>
          )[field] ?? field}: {message}
        </li>{/each}
    </ul>{/if}
  {#if loading}<p role="status">Sprawdzamy konto…</p>
  {:else if !user}<p>
      Możesz poprosić o bezpłatną wycenę bez konta, pisząc na
      <a href="mailto:piotr@rapidmaker.pl?subject=Wycena%20druku%203D">piotr@rapidmaker.pl</a>.
      Jeśli chcesz otrzymać i opłacić ofertę w panelu, zaloguj się, aby przypisać pliki do konta.
    </p>
    <div>
      <a class="button primary" href="/logowanie?next=%2Fzamowienie"
        >Zaloguj się i wróć do zapytania</a
      >
    </div>
  {:else}
    <form
      class="stack"
      onsubmit={(event) => {
        event.preventDefault();
        void send();
      }}
    >
      <fieldset disabled={busy || !!frozen} class="stack">
        <div class="form-grid">
          <label
            >Imię i nazwisko<input
              autocomplete="shipping name"
              bind:value={contact.name}
              maxlength="120"
              required
            /></label
          ><label>Email konta<input type="email" value={user.email} readonly /></label><label
            >Telefon<input
              type="tel"
              autocomplete="shipping tel"
              bind:value={contact.phone}
              maxlength="30"
              required
            /></label
          >
        </div>
        <fieldset>
          <legend>Sposób dostawy do uzgodnienia</legend>
          <div class="methods">
            {#each [{ id: 'courier', name: 'Kurier' }, { id: 'locker', name: 'Punkt odbioru' }, { id: 'pickup', name: 'Odbiór osobisty' }] as method}<label
                class="method"
                ><input
                  type="radio"
                  name="manual-delivery"
                  value={method.id}
                  bind:group={delivery}
                  onchange={() => (contact.pickupPoint = '')}
                />{method.name}</label
              >{/each}
          </div>
        </fieldset>
        {#if delivery !== 'pickup' || invoice}<div class="form-grid">
            <label
              >Kod pocztowy<input
                autocomplete="shipping postal-code"
                pattern={'[0-9]{2}-[0-9]{3}'}
                bind:value={contact.postalCode}
                oninput={() => {
                  contact.pickupPoint = '';
                  points = [];
                  pointGeneration++;
                  pointBusy = false;
                }}
                required
              /></label
            ><label
              >Miejscowość<input
                autocomplete="shipping address-level2"
                bind:value={contact.city}
                maxlength="120"
                required
              /></label
            >
          </div>
          <label
            >Ulica, numer domu i mieszkania<input
              autocomplete="shipping street-address"
              bind:value={contact.street}
              maxlength="200"
              required
            /></label
          >{/if}
        {#if delivery === 'locker'}<div>
            <button
              type="button"
              class="button secondary"
              disabled={pointBusy}
              onclick={searchPoints}
              >{pointBusy ? 'Szukamy punktów…' : 'Znajdź punkty przy podanym adresie'}</button
            >
          </div>
          {#if contact.pickupPoint}<p>Wybrany punkt: <strong>{contact.pickupPoint}</strong></p>{/if}
          <ul class="points">
            {#each points.slice(0, 12) as point}<li>
                <button
                  type="button"
                  class="point"
                  aria-pressed={contact.pickupPoint === point.code}
                  onclick={() => (contact.pickupPoint = point.code)}
                  ><strong>{point.name} · {point.code}</strong><span
                    >{point.address.street}, {point.address.postcode} {point.address.city}</span
                  ></button
                >
              </li>{/each}
          </ul>{/if}
        <label class="invoice"
          ><input type="checkbox" bind:checked={invoice} />Chcę fakturę na firmę</label
        >{#if invoice}<div class="form-grid">
            <label>Nazwa firmy<input bind:value={contact.company} maxlength="200" required /></label
            ><label
              >NIP<input
                inputmode="numeric"
                pattern={'[0-9]{10}'}
                bind:value={contact.nip}
                maxlength="10"
                required
              /></label
            >
          </div>{/if}
        <label
          >Uwagi i wymagania<textarea
            rows="4"
            bind:value={notes}
            maxlength="4000"
            placeholder="Ważne powierzchnie, zastosowanie, termin…"></textarea></label
        >
      </fieldset>
      {#if frozen && !busy}<p class="muted">
          Sprawdź lub ponów to samo zgłoszenie. Jego dane pozostają zablokowane, aby uniknąć
          duplikatu.
        </p>{/if}
      <div>
        <button class="button primary" disabled={busy}
          >{busy
            ? 'Wysyłamy…'
            : frozen
              ? 'Ponów to samo zgłoszenie'
              : 'Wyślij zapytanie o wycenę'}</button
        >
      </div>
      <small
        >Gotową ofertę zobaczysz w zamówieniach. Wyślemy też powiadomienie email. Dopiero
        zaakceptowaną cenę opłacisz tutaj przez Stripe.</small
      >
    </form>
  {/if}
</section>

<style>
  .manual-request {
    max-width: 900px;
  }
  h2,
  p {
    margin: 0;
  }
  fieldset {
    border: 0;
    margin: 0;
    padding: 0;
    min-width: 0;
  }
  label {
    display: grid;
    gap: var(--space, 8px);
    font-size: 13px;
  }
  input,
  textarea {
    min-width: 0;
    width: 100%;
    padding: calc(var(--space, 8px) * 1.5);
    border: 1px solid var(--control-line);
    border-radius: var(--control-radius, var(--radius-sm, 3px));
    background: var(--soft);
    color: var(--ink);
    font: inherit;
  }
  legend {
    font-size: 13px;
    margin-bottom: 12px;
  }
  .methods {
    display: flex;
    flex-wrap: wrap;
    gap: calc(var(--space, 8px) * 2);
  }
  .method,
  .invoice {
    display: flex;
    align-items: center;
    gap: var(--space, 8px);
  }
  .method input,
  .invoice input {
    width: 18px;
  }
  .points {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .point {
    display: grid;
    gap: var(--space, 8px);
    text-align: left;
    width: 100%;
    padding: calc(var(--space, 8px) * 1.5);
    border: 0;
    background: transparent;
    color: var(--ink);
    cursor: pointer;
  }
  .point[aria-pressed='true'] {
    background: var(--soft);
    color: var(--accent);
  }
  .field-errors {
    color: var(--danger);
    font-size: 12px;
  }
  small {
    color: var(--muted);
    line-height: 1.6;
  }
</style>
