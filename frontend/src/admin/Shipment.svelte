<script lang="ts">
  import { ApiError } from '../lib/http';
  import {
    downloadLabel,
    shippingIntent,
    shippingRequest,
    type Parcel,
    type PickupDate,
    type ShipmentAction,
    type ShipmentSnapshot,
    type ShippingIntent,
  } from '../lib/shipping';

  let { orderId, onchange }: { orderId: string; onchange?: () => void } = $props();
  let data = $state<ShipmentSnapshot | null>(null);
  let parcels = $state<Parcel[]>([]);
  let actions = $state<ShipmentAction[]>([]);
  let intent = $state<ShippingIntent | null>(null);
  let selected = $state<string[]>([]);
  let error = $state('');
  let notice = $state('');
  let busy = $state(false);
  let loading = $state(true);
  let reviewed = $state(false);
  let accepted = $state(false);
  let dirty = $state(false);
  let readyDate = $state('');
  let dates = $state<PickupDate[]>([]);
  let chosen = $state('');
  let repeat = $state(false);
  let cancelConfirmed = $state(false);
  let generation = 0;
  const shipment = $derived(data?.shipment);
  const pickupOnly = $derived(
    (typeof data?.order.fulfillment.delivery === 'string'
      ? data.order.fulfillment.delivery
      : data?.order.fulfillment.delivery.method) === 'pickup',
  );
  const editable = $derived(
    !intent &&
      data?.order.status === 'ready' &&
      data.order.paid_at !== null &&
      (!shipment || shipment.state === 'draft'),
  );
  const activeAction = $derived(
    actions.some((a) => ['queued', 'submitting', 'pending', 'manual_review'].includes(a.state)),
  );
  const selectedPackages = $derived(
    shipment?.packages.filter((p) => selected.includes(p.id)) ?? [],
  );
  const money = (n: number) =>
    new Intl.NumberFormat('pl-PL', { style: 'currency', currency: 'PLN' }).format(n / 100);
  const names: Record<string, string> = {
    draft: 'Paczki do potwierdzenia',
    queued: 'Zlecenie zapisane',
    creating: 'Tworzenie przesyłki',
    dispatched: 'Przekazano kurierowi',
    ordering: 'Zamawianie przewozu',
    booked: 'Przewóz zamówiony',
    shipped: 'Przekazano kurierowi',
    delivered: 'Doręczono',
    canceled: 'Anulowano',
    cancelled: 'Anulowano',
    failed: 'Operacja odrzucona',
    manual_review: 'Wymaga wyjaśnienia',
    submitting: 'Przetwarzanie',
    pending: 'Oczekiwanie na potwierdzenie',
    succeeded: 'Potwierdzono',
    partial: 'Częściowo wykonano',
    ordered: 'Zamówiono',
    tracking: 'W drodze',
  };
  const stateName = (state: string) => names[state] ?? state;
  const describe = (e: unknown) =>
    e instanceof Error ? e.message : 'Nie udało się połączyć z serwerem.';

  $effect(() => {
    const id = orderId;
    const current = ++generation;
    data = null;
    actions = [];
    parcels = [];
    selected = [];
    dates = [];
    chosen = '';
    error = '';
    notice = '';
    dirty = false;
    accepted = false;
    busy = false;
    void refresh(id, current);
    return () => {
      generation++;
    };
  });
  async function refresh(id = orderId, current = generation) {
    loading = true;
    try {
      const snapshot = await shippingRequest<ShipmentSnapshot>(id);
      if (!snapshot.order.fulfillment)
        throw new Error(
          'To historyczne zamówienie nie ma zapisanych danych dostawy. Wymaga obsługi ręcznej.',
        );
      const history = snapshot.shipment
        ? await shippingRequest<ShipmentAction[]>(id, '/actions')
        : [];
      if (current !== generation) return;
      data = snapshot;
      actions = history;
      intent = shippingIntent(id);
      if (intent && history.some((a) => a.request_id === intent?.body.request_id)) {
        shippingIntent(id, null);
        intent = null;
      }
      const proposed = snapshot.order.fulfillment.packing?.parcels ?? [];
      parcels = structuredClone(
        snapshot.shipment?.input.parcels ??
          proposed.map((p) => ({
            outer_mm: p.outer_mm ?? p.dimensions_mm ?? [0, 0, 0],
            weight_g: p.weight_g,
          })),
      );
      reviewed = snapshot.shipment?.input.refund_reviewed ?? false;
      dirty = false;
      accepted = false;
      selected = selected.filter((id) => snapshot.shipment?.packages.some((p) => p.id === id));
      if (
        intent?.operation === 'book' &&
        snapshot.shipment &&
        snapshot.shipment.state !== 'draft'
      ) {
        shippingIntent(id, null);
        intent = null;
      }
    } catch (e) {
      if (current === generation) error = describe(e);
    } finally {
      if (current === generation) loading = false;
    }
  }
  async function run(task: () => Promise<void>) {
    if (busy) return;
    busy = true;
    error = '';
    notice = '';
    const current = generation;
    try {
      await task();
    } catch (e) {
      if (current === generation) error = describe(e);
    } finally {
      if (current === generation) busy = false;
    }
  }
  function changed() {
    dirty = true;
    accepted = false;
  }
  async function prepare() {
    if (
      !parcels.length ||
      parcels.some(
        (p) => ![...p.outer_mm, p.weight_g].every((v) => Number.isSafeInteger(v) && v > 0),
      )
    ) {
      error = 'Podaj dodatnie, całkowite wymiary w mm i masę w gramach dla każdej paczki.';
      return;
    }
    const id = orderId,
      current = generation;
    await run(async () => {
      await shippingRequest(id, '', 'PUT', {
        version: shipment?.version ?? 0,
        parcels,
        refund_reviewed: reviewed,
      });
      if (current !== generation) return;
      notice = 'Paczki zapisane. Sprawdź aktualny koszt przewozu.';
      await refresh(id, current);
    });
  }
  async function quote() {
    const id = orderId,
      current = generation,
      version = shipment?.version;
    await run(async () => {
      await shippingRequest(id, '/quote', 'POST', { version });
      if (current === generation) await refresh(id, current);
    });
  }
  async function submit(operation: ShippingIntent['operation'], extra: Record<string, unknown>) {
    const id = orderId,
      current = generation;
    await run(async () => {
      if (!intent) {
        const pending = {
          operation,
          body: { version: shipment?.version, request_id: crypto.randomUUID(), ...extra },
        };
        shippingIntent(id, pending);
        intent = pending;
      }
      const pending = intent;
      try {
        await shippingRequest(id, `/${pending.operation}`, 'POST', pending.body);
      } catch (e) {
        if (e instanceof ApiError && e.status >= 400 && e.status < 500) {
          shippingIntent(id, null);
          if (current === generation) {
            intent = null;
            await refresh(id, current);
          }
        }
        throw e;
      }
      shippingIntent(id, null);
      if (current !== generation) return;
      intent = null;
      accepted = false;
      cancelConfirmed = false;
      notice = 'Zlecenie zapisane. Odśwież, aby sprawdzić potwierdzenie przewoźnika.';
      await refresh(id, current);
      onchange?.();
    });
  }
  async function proposals() {
    const id = orderId,
      current = generation,
      ids = selectedPackages.map((p) => p.provider_id);
    await run(async () => {
      const result = await shippingRequest<{
        packages: {
          package_id: string | number;
          proposals: (PickupDate & { available: boolean })[];
        }[];
      }>(id, '/pickup/proposals', 'POST', { package_ids: selected, ready_date: readyDate, repeat });
      if (current !== generation) return;
      const groups = result.packages ?? [];
      dates = (groups.find((g) => String(g.package_id) === ids[0])?.proposals ?? [])
        .filter(
          (p) =>
            p.available &&
            ids.every((provider) =>
              groups.some(
                (g) =>
                  String(g.package_id) === provider &&
                  g.proposals.some(
                    (x) =>
                      x.available &&
                      x.date === p.date &&
                      x.min_time === p.min_time &&
                      x.max_time === p.max_time,
                  ),
              ),
            ),
        )
        .map(({ date, min_time, max_time }) => ({ date, min_time, max_time }));
      chosen = '';
      if (!dates.length) notice = 'Brak wspólnego terminu. Wybierz inną datę.';
    });
  }
  async function handover() {
    if (!confirm('Potwierdzasz, że zaznaczone przesyłki faktycznie przekazano przewoźnikowi?'))
      return;
    const id = orderId,
      current = generation;
    await run(async () => {
      await shippingRequest(id, '/handover', 'POST', { package_ids: selected });
      if (current === generation) {
        await refresh(id, current);
        onchange?.();
      }
    });
  }
</script>

<section class="shipping" aria-label="Dostawa zamówienia" aria-busy={busy || loading}>
  <header>
    <h3>Dostawa</h3>
    <button class="text-button" disabled={busy || loading} onclick={() => refresh()}>Odśwież</button
    >
  </header>
  {#if error}<p class="problem" role="alert">{error}</p>{/if}
  {#if notice}<p class="hint" role="status">{notice}</p>{/if}
  {#if loading}<p class="muted">Pobieranie dostawy…</p>
  {:else if data}
    {#if pickupOnly}<p>Odbiór osobisty — bez nadawania przesyłki.</p>
    {:else}
      <p>
        <strong>{data.order.fulfillment.offer?.name ?? 'Wybrana usługa'}</strong><small
          >{data.order.contact.name} · {data.order.contact.street} · {data.order.contact
            .postalCode ?? data.order.contact.postal_code}
          {data.order.contact.city}</small
        >
      </p>
      <p class="hint">
        {shipment
          ? stateName(shipment.state)
          : 'Nadanie wymaga opłaconego zamówienia w stanie „Gotowe”.'}
      </p>
      {#if shipment?.last_error}<p class="problem">
          Kod błędu przewoźnika: {shipment.last_error}. Odśwież i wyjaśnij stan przed kolejnym
          nadaniem.
        </p>{/if}
      {#if intent}
        <div class="approval">
          <p>
            Wynik poprzedniej operacji jest niepewny. Ponowienie użyje tego samego identyfikatora i
            danych.
          </p>
          <button class="secondary" disabled={busy} onclick={() => submit(intent!.operation, {})}
            >Sprawdź / ponów tę samą operację</button
          >
        </div>
      {/if}
      <form
        onsubmit={(e) => {
          e.preventDefault();
          void prepare();
        }}
      >
        <h4>
          {shipment ? 'Paczki do nadania' : 'Szacowane paczki'}
          <small>Wymiary zewnętrzne w mm, masa razem z opakowaniem w g.</small>
        </h4>
        {#each parcels as p, i}
          {#if !editable}<div class="cost">
              <strong>Paczka {i + 1}</strong><span
                >{p.outer_mm.join(' × ')} mm · {p.weight_g} g</span
              >
            </div>
          {:else}<fieldset disabled={busy || !editable}>
              <legend>Paczka {i + 1}</legend>
              <div class="dimensions">
                {#each ['Długość', 'Szerokość', 'Wysokość'] as name, axis}<label
                    >{name}<input
                      type="number"
                      min="1"
                      step="1"
                      required
                      bind:value={p.outer_mm[axis]}
                      oninput={changed}
                    /></label
                  >{/each}
                <label
                  >Masa<input
                    type="number"
                    min="1"
                    step="1"
                    required
                    bind:value={p.weight_g}
                    oninput={changed}
                  /></label
                >
              </div>
              <button
                class="text-button"
                type="button"
                onclick={() => {
                  parcels = parcels.filter((_, n) => n !== i);
                  changed();
                }}>Usuń paczkę</button
              >
            </fieldset>{/if}
        {/each}
        {#if editable}
          <button
            class="text-button"
            type="button"
            disabled={busy || parcels.length >= 30}
            onclick={() => {
              parcels = [...parcels, { outer_mm: [0, 0, 0], weight_g: 0 }];
              changed();
            }}>+ Dodaj paczkę</button
          >
          <label class="check"
            ><input
              type="checkbox"
              bind:checked={reviewed}
              onchange={changed}
              disabled={busy}
            />Potwierdzam zakres wysyłki po częściowym zwrocie, jeśli wystąpił.</label
          >
          <div class="buttons">
            <button class="secondary" disabled={busy || !parcels.length}>Zapisz paczki</button
            ><button
              class="secondary"
              type="button"
              disabled={busy || dirty || !shipment}
              onclick={quote}>Sprawdź koszt przewozu</button
            >
          </div>
        {/if}
      </form>
      {#if shipment?.quote && shipment.state === 'draft' && !dirty && !intent}
        {@const q = shipment.quote}
        <div class="approval">
          <div class="cost">
            <span>Aktualny koszt przewozu</span><strong>{money(q.price_grosze)}</strong>
          </div>
          <div class="cost muted">
            <span>Pobrano od klienta</span><span>{money(q.collected_shipping_grosze)}</span>
          </div>
          <div class="cost muted">
            <span>Różnica kosztu</span><span>{money(q.difference_grosze)}</span>
          </div>
          <small
            >Oferta do {new Date(q.expires_at * 1000).toLocaleTimeString('pl-PL')}. Bez
            automatycznej dopłaty klienta. Podjazd zamawia się osobno.</small
          >
          <label class="check"
            ><input type="checkbox" bind:checked={accepted} disabled={busy} />Akceptuję koszt {money(
              q.price_grosze,
            )} i zamówienie przewozu.</label
          >
          <button
            class="primary"
            disabled={busy || !accepted}
            onclick={() => {
              if (q.expires_at * 1000 <= Date.now()) {
                error = 'Oferta wygasła. Sprawdź koszt ponownie.';
                accepted = false;
                return;
              }
              void submit('book', { quote_id: q.id });
            }}>Zamów przewóz za {money(q.price_grosze)}</button
          >
        </div>
      {/if}
      {#if shipment?.packages.length}
        <h4>Przesyłki i etykiety</h4>
        {#each shipment.packages as p, i}
          <div class="package">
            <label class="check"
              ><input
                type="checkbox"
                value={p.id}
                bind:group={selected}
                disabled={busy ||
                  !!intent ||
                  activeAction ||
                  !p.provider_id ||
                  p.state === 'canceled'}
                onchange={() => {
                  dates = [];
                  chosen = '';
                  cancelConfirmed = false;
                }}
              /><span
                >Przesyłka {i + 1}<small
                  >{stateName(p.state)} · {p.provider_id ?? 'Oczekiwanie na numer'}</small
                ></span
              ></label
            ><button
              class="text-button"
              disabled={busy || !p.provider_id || activeAction}
              onclick={() => run(() => downloadLabel(orderId, p.id))}>Etykieta PDF</button
            >
          </div>
        {/each}
        <div class="buttons">
          <button
            class="secondary"
            disabled={busy ||
              !selected.length ||
              !!intent ||
              activeAction ||
              shipment.state !== 'booked'}
            onclick={handover}>Potwierdź przekazanie kurierowi</button
          >
        </div>
        {#if selected.length && !intent && !activeAction}
          <details>
            <summary>Podjazd kuriera</summary>
            <label
              >Gotowe do odbioru od<input
                type="date"
                bind:value={readyDate}
                onchange={() => {
                  dates = [];
                  chosen = '';
                }}
                disabled={busy}
              /></label
            >
            <label class="check"
              ><input
                type="checkbox"
                bind:checked={repeat}
                onchange={() => {
                  dates = [];
                  chosen = '';
                }}
                disabled={busy}
              />Ponowny podjazd po nieudanym odbiorze</label
            >
            <button class="secondary" disabled={busy || !readyDate} onclick={proposals}
              >Pokaż dostępne terminy</button
            >
            {#if dates.length}<label
                >Termin<select bind:value={chosen} disabled={busy}
                  ><option value="">Wybierz termin</option>{#each dates as d, i}<option
                      value={String(i)}>{d.date} · {d.min_time}–{d.max_time}</option
                    >{/each}</select
                ></label
              ><button
                class="primary"
                disabled={busy || chosen === ''}
                onclick={() =>
                  submit('pickup', {
                    package_ids: selected,
                    pickup_date: dates[Number(chosen)],
                    repeat,
                  })}>Zamów podjazd</button
              >{/if}
          </details>
          <details>
            <summary>Anulowanie zaznaczonych przesyłek</summary>
            <p class="hint">
              Nie anuluje zamówienia ani płatności. Przewoźnik może naliczyć opłatę.
            </p>
            <label class="check"
              ><input type="checkbox" bind:checked={cancelConfirmed} disabled={busy} />Potwierdzam
              anulowanie {selected.length} przesyłek.</label
            ><button
              class="secondary"
              disabled={busy || !cancelConfirmed}
              onclick={() => submit('cancel', { package_ids: selected })}>Zleć anulowanie</button
            >
          </details>
        {/if}
      {/if}
      {#if actions.length}<h4>Operacje przewoźnika</h4>
        {#each actions as action}<div class="cost">
            <span>{action.kind === 'pickup' ? 'Podjazd kuriera' : 'Anulowanie'}</span><strong
              >{stateName(action.state)}</strong
            >
          </div>{/each}{/if}
    {/if}
  {/if}
</section>

<style>
  .shipping {
    display: grid;
    gap: 18px;
    font-size: 12px;
    min-width: 0;
  }
  header,
  .cost,
  .package {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
  }
  h4 {
    font-size: 13px;
    font-weight: 650;
    margin: 0 0 12px;
  }
  form {
    display: grid;
    gap: 12px;
  }
  fieldset {
    border: 0;
    padding: 0;
    margin: 0 0 12px;
    min-width: 0;
  }
  legend {
    font-size: 11px;
    color: var(--muted);
    margin-bottom: 8px;
  }
  .dimensions {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
  }
  label {
    display: grid;
    gap: 6px;
    font-size: 11px;
  }
  input,
  select {
    font-size: 12px;
    padding: 9px 8px;
  }
  .check {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    line-height: 1.6;
  }
  .check input {
    width: 15px;
    height: 15px;
    flex: 0 0 15px;
    margin: 3px 0 0;
    accent-color: var(--accent);
  }
  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .approval {
    background: var(--soft);
    border: 1px solid var(--control-line);
    padding: 16px;
    display: grid;
    gap: 14px;
    border-radius: 3px;
  }
  .hint {
    color: var(--muted);
    line-height: 1.6;
  }
  .problem {
    color: var(--ink);
    border-left: 3px solid var(--accent);
    padding-left: 12px;
    overflow-wrap: anywhere;
  }
  .package {
    padding: 10px 0;
  }
  .package > label {
    flex: 1;
    min-width: 0;
  }
  .package small {
    overflow-wrap: anywhere;
  }
  details {
    padding: 10px 0;
  }
  summary {
    cursor: pointer;
    font-weight: 650;
    margin-bottom: 12px;
  }
  details label,
  details p,
  details button {
    margin-bottom: 12px;
  }
  @media (max-width: 420px) {
    .dimensions {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .package {
      align-items: flex-start;
    }
    .buttons button {
      width: 100%;
    }
  }
</style>
