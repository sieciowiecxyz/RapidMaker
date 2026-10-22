<script lang="ts">
  import { onMount } from 'svelte';
  import * as api from '../../lib/api';
  import { money } from '../../lib/format';
  import type { Cart, Material, PrintModel, PrintOptionsCatalog } from '../../lib/types';
  import CheckoutSteps from '../../components/CheckoutSteps.svelte';
  import AdvancedOptions from './AdvancedOptions.svelte';
  import PrintChoices from './PrintChoices.svelte';
  let printOptions = $state<PrintOptionsCatalog | null>(null);
  let cart = $state<Cart | null>(null),
    materials = $state<Material[]>([]);
  let busy = $state(true),
    error = $state(''),
    progress = $state(''),
    selectedId = $state('');
  let generation = 0;
  let quoteValid = $state(false);
  let selected = $derived(cart?.models.find((model) => model.id === selectedId) ?? cart?.models[0]);
  onMount(() => {
    refresh();
  });
  async function refresh() {
    const mine = ++generation;
    busy = true;
    quoteValid = false;
    error = '';
    try {
      const [next, nextMaterials, nextPrintOptions] = await Promise.all([
        api.getCart(),
        api.getMaterials(),
        api.getPrintOptions(),
      ]);
      if (mine !== generation) return;
      cart = next;
      materials = nextMaterials;
      printOptions = nextPrintOptions;
      if (next.models.length) {
        const quote = await api.quotePrintCart();
        if (mine !== generation) return;
        cart = { ...next, quote };
        quoteValid = true;
      }
    } catch (e) {
      if (mine === generation) error = (e as Error).message;
    } finally {
      if (mine === generation) busy = false;
    }
  }
  async function mutate(action: () => Promise<unknown>) {
    ++generation;
    busy = true;
    quoteValid = false;
    error = '';
    try {
      await action();
      await refresh();
    } catch (e) {
      error = (e as Error).message;
      busy = false;
    }
  }
  async function files(list: FileList | null) {
    if (!list || busy) return;
    const chosen = Array.from(list);
    error = '';
    for (const file of chosen) {
      if (!/\.(stl|step|stp|3mf)$/i.test(file.name)) {
        error = `${file.name}: wybierz STL, STEP lub 3MF.`;
        return;
      }
      if (!file.size || file.size > 32 * 1024 * 1024) {
        error = `${file.name}: plik musi mieć od 1 B do 32 MB.`;
        return;
      }
    }
    await mutate(async () => {
      for (let index = 0; index < chosen.length; index++) {
        progress = `Analizujemy model ${index + 1} z ${chosen.length}…`;
        const analysis = await api.analyzeModel(chosen[index]);
        const next = await api.addModel(chosen[index].name);
        selectedId = next.models.at(-1)?.id ?? '';
        await api.updateModel(selectedId, {
          analysis,
          shipping: analysis.weight_g
            ? {
                dimensions_mm: analysis.dimensions_mm.map(Math.ceil) as [number, number, number],
                weight_g: Math.ceil(analysis.weight_g),
              }
            : undefined,
        });
      }
    });
    progress = '';
  }
  function patch(model: PrintModel, changes: Partial<PrintModel>) {
    return mutate(() => api.updateModel(model.id, changes));
  }
  function materialName(model: PrintModel) {
    return (
      materials.find((item) => item.id === model.options.materialId)?.name.replace('Bambu ', '') ??
      ''
    );
  }
</script>

<section class="page purchase-page">
  <CheckoutSteps current={1} />
  <a class="text-button" href="/wycena?restore=cart">← Wróć do konfiguratora</a>
  <div class="page-heading">
    <h1>Opcje zaawansowane druku.</h1>
    <p class="muted">Dodaj części. Wybierz materiał, kolor i sposób wykonania dla każdej z nich.</p>
  </div>
  {#if error}<p class="alert error" role="alert">
      {error} <button class="button ghost" onclick={refresh}>Spróbuj ponownie</button>
    </p>{/if}
  <div class="configure-layout work-surface">
    <div class="config-main">
      <div
        class="upload"
        class:compact={Boolean(cart?.models.length)}
        role="region"
        aria-label="Dodawanie modeli"
        ondragover={(event) => event.preventDefault()}
        ondrop={(event) => {
          event.preventDefault();
          files(event.dataTransfer?.files ?? null);
        }}
      >
        <span class="upload-symbol" aria-hidden="true">↑</span>
        <div class="upload-copy">
          <h2>{cart?.models.length ? 'Dodaj kolejną część' : 'Przeciągnij swoje modele'}</h2>
          <p class="muted">STL, STEP, 3MF · do 32 MB / plik · STL w milimetrach</p>
        </div>
        <label class="button primary upload-label"
          >Wybierz pliki<input
            aria-label="Wybierz modele"
            type="file"
            multiple
            accept=".stl,.step,.stp,.3mf"
            disabled={busy}
            onchange={async (event) => {
              const input = event.currentTarget;
              await files(input.files);
              input.value = '';
            }}
          /></label
        >
        <button
          class="button ghost"
          disabled={busy}
          onclick={() =>
            mutate(async () => {
              const next = await api.addModel('Uchwyt na słuchawki.stl');
              selectedId = next.models.at(-1)!.id;
              await api.updateModel(selectedId, {
                shipping: { dimensions_mm: [120, 80, 45], weight_g: 60 },
              });
            })}>Wypróbuj przykład</button
        >
        {#if progress}<p class="upload-progress muted" role="status">{progress}</p>{/if}
      </div>
      {#if cart?.models.length}
        <div class="parts" role="group" aria-label="Części w zamówieniu">
          {#each cart.models as model, index}
            <button
              class="part-row"
              class:active={selected?.id === model.id}
              aria-pressed={selected?.id === model.id}
              disabled={busy}
              onclick={() => (selectedId = model.id)}
            >
              <span class="part-index">{String(index + 1).padStart(2, '0')}</span>
              <span class="part-name"
                ><strong>{model.name}</strong><small
                  >{materialName(model)} · {model.options.color}</small
                ></span
              >
              <span class="part-quantity">{model.options.quantity} szt.</span><span
                class="part-action">{selected?.id === model.id ? 'Edytujesz' : 'Zmień →'}</span
              >
            </button>
          {/each}
        </div>
      {/if}
      {#if selected}
        {@const model = selected}
        <article class="model-editor">
          <div class="editor-heading">
            <div>
              <span class="eyebrow">Ustawienia wybranej części · {model.format}</span>
              <h2>{model.name}</h2>
            </div>
            <button
              class="button ghost"
              disabled={busy}
              onclick={() => mutate(() => api.removeModel(model.id))}
              aria-label={`Usuń ${model.name}`}>Usuń</button
            >
          </div>
          <div class="model-intro">
            <div class="preview">
              <svg viewBox="0 0 120 90" role="img" aria-label="Miejsce na podgląd modelu"
                ><path
                  d="m60 12 40 22v40L60 94 20 74V34z"
                  transform="translate(0 -8)"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.2"
                /><path
                  d="m20 26 40 23 40-23M60 49v37"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.2"
                /></svg
              >
              <div>
                <strong>Podgląd modelu</strong><span
                  >{model.analysis
                    ? model.analysis.dimensions_mm.map((v) => v.toFixed(1)).join(' × ') + ' mm'
                    : 'Miejsce na Twój model 3D'}</span
                >
                {#if model.analysis}<span
                    >{model.analysis.weight_g
                      ? 'Masa do pakowania: do ' +
                        Math.ceil(model.analysis.weight_g) +
                        ' g (pełna bryła)'
                      : (model.analysis.reason ?? 'Masa wymaga analizy.')}</span
                  >{/if}
              </div>
            </div>
            <label class="quantity"
              >Liczba sztuk<input
                type="number"
                min="1"
                max="1000"
                step="1"
                disabled={busy}
                value={model.options.quantity}
                onchange={(event) =>
                  patch(model, {
                    options: { ...model.options, quantity: Number(event.currentTarget.value) },
                  })}
              /><small>Ta sama konfiguracja dla każdej sztuki.</small></label
            >
          </div>
          {#if printOptions}
            {#key model.id}<PrintChoices
                {model}
                {materials}
                catalog={printOptions}
                {busy}
                onchange={(changes) => patch(model, changes)}
              />{/key}
            <AdvancedOptions
              {model}
              catalog={printOptions}
              {busy}
              onchange={(changes) => patch(model, changes)}
            />
          {/if}
        </article>
      {/if}
    </div>
    <aside class="summary" aria-busy={busy}>
      <span class="eyebrow">Twoje części</span>
      <h2>Wydruk</h2>
      {#if cart?.models.length}
        <div class="summary-parts">
          {#each cart.models as model}<div class="line">
              <span
                >{model.name}<small
                  >{materialName(model)} · {model.options.quality === 'fine'
                    ? 'Dokładna'
                    : model.options.quality === 'draft'
                      ? 'Robocza'
                      : 'Standard'}</small
                ></span
              ><strong>{model.options.quantity} szt.</strong>
            </div>{/each}
        </div>
        <div class="order-actions">
          <div class="total">
            <span>Wydruk i przygotowanie</span><strong
              >{cart.quote ? money(cart.quote.subtotal) : '—'}</strong
            >
          </div>
          {#if busy || !quoteValid}<button class="button primary" disabled
              >{busy ? 'Przeliczamy…' : 'Wycena niedostępna'}</button
            >{:else}<a class="button primary" href="/zamowienie">Dalej: wysyłka →</a>{/if}
        </div>
        <p class="muted">Dostawa w następnym kroku.</p>
        {#if cart.quote?.manual}<p class="alert">
            Nietypowe wymagania potwierdzi zespół. {cart.quote.reason}
          </p>{/if}
        <p class="quote-state" aria-live="polite">
          {busy
            ? 'Aktualizujemy cenę…'
            : !quoteValid
              ? 'Nie udało się uzyskać aktualnej wyceny.'
              : 'Cena testowa — docelowo zależna od parametrów.'}
        </p>
        <p class="guest-note">Możesz zamówić bez zakładania konta.</p>
      {:else}<p class="muted">Dodaj model, aby skonfigurować wydruk.</p>{/if}
    </aside>
  </div>
</section>

<style>
  .eyebrow {
    color: var(--color-muted);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
  .configure-layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 260px;
    gap: 36px;
    align-items: start;
  }
  .config-main {
    min-width: 0;
  }
  .upload {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 14px;
    padding: 28px 0;
    border-bottom: 1px solid var(--color-border);
  }
  .upload-copy {
    flex: 1;
    min-width: 190px;
  }
  .upload h2 {
    font-size: 17px;
    margin: 0 0 6px;
  }
  .upload p {
    font-size: 11px;
    margin: 0;
  }
  .upload-symbol {
    font-size: 26px;
    color: var(--color-accent);
  }
  .upload.compact {
    padding: 18px 0;
  }
  .upload.compact .upload-symbol {
    display: none;
  }
  .upload-label {
    position: relative;
    overflow: hidden;
    cursor: pointer;
  }
  .upload-label:focus-within {
    outline: 2px solid var(--color-focus);
    outline-offset: 4px;
  }
  .upload-label input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
  }
  .upload-progress {
    flex-basis: 100%;
  }
  .parts {
    margin-top: 20px;
  }
  .part-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 13px;
    padding: 15px 12px;
    border: 0;
    border-bottom: 1px solid var(--color-border);
    background: transparent;
    color: var(--color-text);
    text-align: left;
    cursor: pointer;
    font: inherit;
    border-left: 2px solid transparent;
  }
  .part-row:hover {
    background: var(--color-option);
  }
  .part-row.active {
    border-left-color: var(--color-accent);
    background: var(--color-selected);
  }
  .part-index {
    color: var(--color-muted);
    font-size: 11px;
  }
  .part-name {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 5px;
  }
  .part-name strong {
    font-size: 13px;
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .part-name small {
    color: var(--color-muted);
    font-size: 10px;
  }
  .part-quantity {
    white-space: nowrap;
    font-size: 12px;
  }
  .part-action {
    color: var(--color-muted);
    font-size: 10px;
    min-width: 56px;
    text-align: right;
  }
  .active .part-action {
    color: var(--color-accent);
  }
  .model-editor {
    padding: 28px 0;
  }
  .editor-heading {
    display: flex;
    gap: 16px;
    justify-content: space-between;
    align-items: start;
  }
  .editor-heading > div {
    min-width: 0;
  }
  .editor-heading h2 {
    font-size: 21px;
    margin: 7px 0 0;
    overflow-wrap: anywhere;
    letter-spacing: -0.02em;
  }
  .model-intro {
    display: grid;
    grid-template-columns: 1fr 200px;
    gap: 28px;
    align-items: center;
    padding: 23px 0;
    margin-bottom: 24px;
    border-bottom: 1px solid var(--color-border);
  }
  .preview {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--color-muted);
  }
  .preview svg {
    flex: 0 0 70px;
    height: 65px;
    color: var(--color-accent);
    opacity: 0.65;
  }
  .preview div {
    display: grid;
    gap: 5px;
  }
  .preview strong {
    font-size: 12px;
    font-weight: 500;
    color: var(--color-text);
  }
  .preview span {
    font-size: 10px;
  }
  .quantity {
    display: grid;
    gap: 7px;
    font-size: 12px;
  }
  .quantity input {
    width: 100%;
  }
  .quantity small {
    color: var(--color-muted);
    font-size: 10px;
  }
  .summary {
    position: sticky;
    top: 28px;
    border-left: 1px solid var(--color-border);
    padding: 6px 0 6px 24px;
  }
  .summary h2 {
    font-size: 23px;
    margin: 8px 0 20px;
  }
  .summary-parts {
    margin-bottom: 22px;
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 14px;
    padding: 13px 0;
    font-size: 13px;
  }
  .line > span {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .line small {
    display: block;
    color: var(--color-muted);
    font-size: 12px;
    margin-top: 6px;
  }
  .line strong {
    white-space: nowrap;
    font-weight: 400;
  }
  .total {
    display: grid;
    gap: 8px;
  }
  .total span {
    color: var(--color-muted);
    font-size: 11px;
  }
  .total strong {
    color: var(--color-heading);
    font-size: 32px;
    letter-spacing: -0.04em;
    font-weight: 500;
  }
  .summary > .muted {
    font-size: 11px;
    line-height: 1.6;
    margin: 9px 0 18px;
  }
  .summary .button {
    width: 100%;
  }
  .quote-state,
  .guest-note {
    color: var(--color-muted);
    font-size: 12px;
    line-height: 1.6;
    margin: 14px 0;
  }
  @media (max-width: 1100px) {
    .configure-layout {
      gap: 32px;
      grid-template-columns: minmax(0, 1fr) 250px;
    }
  }
  @media (max-width: 850px) {
    .configure-layout {
      grid-template-columns: 1fr;
      gap: 16px;
    }
    .summary {
      position: static;
      padding: 24px 0 0;
      border-left: 0;
      border-top: 1px solid var(--color-border);
    }
    .model-intro {
      grid-template-columns: 1fr 150px;
      gap: 14px;
    }
    .part-action {
      display: none;
    }
  }
  @media (max-width: 420px) {
    .model-intro {
      grid-template-columns: 1fr;
    }
    .preview svg {
      width: 48px;
      height: 48px;
      flex-basis: 48px;
    }
    .quantity {
      grid-template-columns: 1fr 100px;
      align-items: center;
    }
    .quantity small {
      grid-column: 1 / -1;
    }
    .upload .button {
      flex: 1;
    }
    .editor-heading h2 {
      font-size: 18px;
    }
  }
</style>
