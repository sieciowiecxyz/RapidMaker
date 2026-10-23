<script lang="ts">
  import { onMount } from 'svelte';
  import {
    operator,
    hundredths,
    type OperatorMaterial,
    type OperatorColor,
  } from '../../lib/operator';
  import Button from '../../components/ui/Button.svelte';

  type Draft = {
    id: string;
    name: string;
    density: string;
    price: string;
    enabled: boolean;
    colors: OperatorColor[];
    newColor: OperatorColor;
  };
  const emptyColor = (): OperatorColor => ({
    code: '',
    name: '',
    rgba: ['#FFFFFFFF'],
    in_stock: false,
  });
  const draft = (m: OperatorMaterial): Draft => ({
    id: m.id,
    name: m.name,
    density: String(m.density_g_cm3),
    price: m.cost_net_grosze_per_kg === null ? '' : (m.cost_net_grosze_per_kg / 100).toFixed(2),
    enabled: m.production_enabled,
    colors: m.colors.map((c) => ({ ...c, rgba: [...c.rgba] })),
    newColor: emptyColor(),
  });
  let materials = $state<OperatorMaterial[]>([]);
  let drafts = $state<Record<string, Draft>>({});
  const emptyMaterial = (): Draft => ({
    id: '',
    name: '',
    density: '',
    price: '',
    enabled: false,
    colors: [],
    newColor: emptyColor(),
  });
  let newMaterial = $state<Draft>(emptyMaterial());
  let loading = $state(true);
  let search = $state('');
  let busy = $state(false);
  let feedback = $state('');
  let feedbackId = $state('');
  let failed = $state(false);
  let filtered = $derived(
    materials.filter((m) =>
      `${m.name} ${m.id}`.toLowerCase().includes(search.trim().toLowerCase()),
    ),
  );
  const path = (id: string) => `materials/${encodeURIComponent(id)}`;
  const hasProfile = (m: OperatorMaterial) =>
    Boolean(m.source?.material?.p1p_04_profiles?.length) &&
    !m.source?.material?.base_profile?.startsWith('Bambu Support');
  const swatch = (colors: string[]) =>
    colors.length > 1 ? `linear-gradient(90deg, ${colors.join(',')})` : colors[0];

  async function load() {
    loading = true;
    try {
      materials = (await operator<{ materials: OperatorMaterial[] }>('materials')).materials;
      drafts = Object.fromEntries(materials.map((m) => [m.id, draft(m)]));
      feedback = '';
    } catch (e) {
      feedback = (e as Error).message;
      failed = true;
      feedbackId = '';
    } finally {
      loading = false;
    }
  }
  onMount(load);
  async function write(id: string, action: () => Promise<void>) {
    busy = true;
    feedbackId = id;
    feedback = '';
    failed = false;
    try {
      await action();
      feedback = 'Zapisano.';
    } catch (e) {
      feedback = (e as Error).message;
      failed = true;
    } finally {
      busy = false;
    }
  }
  async function saveMaterial(event: SubmitEvent, item: Draft, creating = false) {
    event.preventDefault();
    await write(creating ? 'new' : item.id, async () => {
      const id = item.id.trim();
      if (!/^[A-Za-z0-9_-]{1,64}$/.test(id))
        throw new Error('ID: użyj 1–64 liter, cyfr, myślników lub podkreśleń.');
      if (creating && materials.some((m) => m.id === id))
        throw new Error('Ten identyfikator już istnieje. Edytuj istniejący materiał.');
      const density = Number(item.density.replace(',', '.'));
      if (!Number.isFinite(density) || density <= 0 || density >= 10)
        throw new Error('Gęstość musi wynosić więcej niż 0 i mniej niż 10 g/cm³.');
      const price = item.price.trim() ? hundredths(item.price, 'Cena zakupu', 100_000_000) : null;
      if (price === 0 || (item.enabled && price === null))
        throw new Error('Dopuszczenie materiału wymaga dodatniej ceny zakupu netto/kg.');
      const input = {
        name: item.name.trim(),
        density_g_cm3: density,
        cost_net_grosze_per_kg: price,
        production_enabled: item.enabled,
      };
      await operator(path(id), input);
      if (creating) {
        const saved: OperatorMaterial = { id, ...input, colors: [] };
        materials = [...materials, saved].sort((a, b) => a.name.localeCompare(b.name));
        drafts = { ...drafts, [id]: draft(saved) };
        newMaterial = emptyMaterial();
      } else {
        materials = materials.map((m) => (m.id === id ? { ...m, ...input } : m));
      }
    });
  }
  async function saveColor(
    event: SubmitEvent,
    material: OperatorMaterial,
    color: OperatorColor,
    creating = false,
  ) {
    event.preventDefault();
    await write(`${material.id}:${creating ? 'new' : color.code}`, async () => {
      const input = {
        ...color,
        code: color.code.trim(),
        name: color.name.trim(),
        rgba: color.rgba.map((r) => r.trim().toUpperCase()),
      };
      if (!/^[A-Za-z0-9_-]{1,64}$/.test(input.code))
        throw new Error('Kod koloru: użyj liter, cyfr, myślników lub podkreśleń.');
      if (creating && material.colors.some((c) => c.code === input.code))
        throw new Error('Ten kod koloru już istnieje.');
      if (!input.name || !input.rgba.length || input.rgba.some((r) => !/^#[0-9A-F]{8}$/.test(r)))
        throw new Error('Podaj nazwę oraz kolor w formacie #RRGGBBAA.');
      await operator(`${path(material.id)}/colors/${encodeURIComponent(input.code)}`, input);
      const colors = creating
        ? [...material.colors, input]
        : material.colors.map((c) => (c.code === input.code ? input : c));
      materials = materials.map((m) => (m.id === material.id ? { ...m, colors } : m));
      if (creating) {
        drafts[material.id].colors.push({ ...input, rgba: [...input.rgba] });
        drafts[material.id].newColor = emptyColor();
      }
    });
  }
</script>

<div class="page stack">
  <div class="page-heading">
    <div>
      <p class="eyebrow">Panel zespołu</p>
      <h1>Materiały i kolory</h1>
      <p class="muted">Katalog Bambu · ceny zakupu netto · dostępność magazynowa</p>
    </div>
  </div>
  <label class="search"
    >Znajdź materiał<input
      bind:value={search}
      type="search"
      placeholder="Nazwa lub identyfikator, np. PLA"
    /></label
  >
  {#if feedback && !feedbackId}<p class="alert" class:error={failed} role="status">
      {feedback}
    </p>{/if}
  {#if loading}<p role="status">Ładowanie katalogu…</p>
  {:else}
    {#if !materials.length && failed}<Button onclick={load}>Spróbuj ponownie</Button>{/if}
    <details class="material add-material">
      <summary><strong>Dodaj materiał</strong><span>+</span></summary>
      <form class="editor" onsubmit={(event) => saveMaterial(event, newMaterial, true)}>
        <div class="form-grid">
          <label
            >Identyfikator<input
              bind:value={newMaterial.id}
              required
              maxlength="64"
              placeholder="BAMBU_CUSTOM"
            /></label
          >
          <label>Nazwa<input bind:value={newMaterial.name} required maxlength="200" /></label>
          <label
            >Gęstość (g/cm³)<input
              bind:value={newMaterial.density}
              inputmode="decimal"
              required
            /></label
          >
          <label
            >Cena zakupu netto (zł/kg)<input
              bind:value={newMaterial.price}
              inputmode="decimal"
              placeholder="Nieustalona"
            /></label
          >
        </div>
        <p class="muted note">
          Nowy materiał wymaga przypisania zaufanego profilu slicera w backendzie. Sam wpis w
          katalogu nie uruchamia wyceny.
        </p>
        <Button type="submit" disabled={busy}>Dodaj materiał</Button>
        {#if feedbackId === 'new' && feedback}<p class:error={failed} role="status">
            {feedback}
          </p>{/if}
      </form>
    </details>
    <div class="materials-list">
      {#each filtered as material (material.id)}
        {@const item = drafts[material.id]}
        <details class="material" name="material-editor">
          <summary>
            <span
              ><strong>{material.name}</strong><small
                >{material.id}{!hasProfile(material) ? ' · bez automatycznej wyceny' : ''}</small
              ></span
            >
            <span class="muted"
              >{material.colors.filter((c) => c.in_stock).length}/{material.colors.length} kolorów</span
            >
            <span class:approved={material.production_enabled} class="status"
              >{material.production_enabled ? 'Dopuszczony' : 'Niedopuszczony'}</span
            >
            <span class="material-price"
              >{material.cost_net_grosze_per_kg === null
                ? 'Brak ceny'
                : `${(material.cost_net_grosze_per_kg / 100).toFixed(2)} zł/kg`}</span
            >
          </summary>
          <div class="editor">
            <form class="stack" onsubmit={(event) => saveMaterial(event, item)}>
              <div class="form-grid">
                <label>Nazwa<input bind:value={item.name} required maxlength="200" /></label>
                <label
                  >Gęstość (g/cm³)<input
                    bind:value={item.density}
                    inputmode="decimal"
                    required
                  /></label
                >
                <label
                  >Cena zakupu netto (zł/kg)<input
                    bind:value={item.price}
                    inputmode="decimal"
                    placeholder="Nieustalona"
                  /></label
                >
              </div>
              <label class="check"
                ><input type="checkbox" bind:checked={item.enabled} />Dopuszczam materiał do
                produkcji na P1P z dyszą hardened steel 0,4 mm.</label
              >
              <p class="muted note">
                Zatwierdź po sprawdzeniu profilu i wymagań sprzętowych. Dysza hardened steel nie
                zapewnia zgodności każdego materiału. Sprzedaż wymaga ceny zakupu, dostępnego koloru
                i obsługiwanego profilu.
              </p>
              <div><Button type="submit" disabled={busy}>Zapisz materiał</Button></div>
            </form>
            <h3>Kolory</h3>
            <div class="colors">
              {#each item.colors as color (color.code)}
                <form class="color-row" onsubmit={(event) => saveColor(event, material, color)}>
                  <span class="swatch" style:background={swatch(color.rgba)} aria-hidden="true"
                  ></span>
                  <label
                    >Nazwa <small>{color.code}</small><input
                      bind:value={color.name}
                      required
                      maxlength="200"
                    /></label
                  >
                  <details class="rgba-editor">
                    <summary>Kolor</summary>{#each color.rgba as rgba, index}<label
                        >RGBA {index + 1}<input
                          bind:value={color.rgba[index]}
                          required
                          pattern={'#[0-9A-Fa-f]{8}'}
                          maxlength="9"
                        /></label
                      >{/each}
                  </details>
                  <label class="check"
                    ><input type="checkbox" bind:checked={color.in_stock} />W magazynie</label
                  >
                  <Button type="submit" variant="secondary" disabled={busy}>Zapisz kolor</Button>
                  {#if feedbackId === `${material.id}:${color.code}` && feedback}<p
                      class="row-feedback"
                      class:error={failed}
                      role="status"
                    >
                      {feedback}
                    </p>{/if}
                </form>
              {:else}<p class="muted">Brak kolorów. Dodaj pierwszy poniżej.</p>{/each}
            </div>
            <details class="add-color">
              <summary>Dodaj kolor</summary>
              <form
                class="stack"
                onsubmit={(event) => saveColor(event, material, item.newColor, true)}
              >
                <div class="form-grid">
                  <label>Kod<input bind:value={item.newColor.code} required maxlength="64" /></label
                  >
                  <label
                    >Nazwa<input bind:value={item.newColor.name} required maxlength="200" /></label
                  >
                  <label
                    >Kolor RGBA<input
                      bind:value={item.newColor.rgba[0]}
                      required
                      pattern={'#[0-9A-Fa-f]{8}'}
                      maxlength="9"
                      placeholder="#FFFFFFFF"
                    /></label
                  >
                </div>
                <label class="check"
                  ><input type="checkbox" bind:checked={item.newColor.in_stock} />W magazynie</label
                >
                <div>
                  <Button type="submit" variant="secondary" disabled={busy}>Dodaj kolor</Button>
                </div>
                {#if feedbackId === `${material.id}:new` && feedback}<p
                    class:error={failed}
                    role="status"
                  >
                    {feedback}
                  </p>{/if}
              </form>
            </details>
            {#if feedbackId === material.id && feedback}<p class:error={failed} role="status">
                {feedback}
              </p>{/if}
          </div>
        </details>
      {:else}<p class="muted">Nie znaleziono materiałów.</p>{/each}
    </div>
  {/if}
</div>

<style>
  .search {
    max-width: 480px;
  }
  .material {
    border-top: 0;
    padding-top: 0;
    border-bottom: 1px solid var(--color-border);
  }
  .material > summary {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto auto;
    align-items: center;
    gap: 20px;
    padding: 20px 0;
    cursor: pointer;
    list-style: none;
    font-size: 13px;
  }
  .material strong {
    font-size: 16px;
    font-weight: 500;
  }
  .material small {
    display: block;
    margin-top: 5px;
    color: var(--color-muted);
  }
  .approved {
    color: var(--color-accent);
  }
  .material-price {
    font-size: 14px;
    font-weight: 500;
    text-align: right;
    min-width: 100px;
  }
  .editor {
    padding: 8px 0 24px;
  }
  .editor h3 {
    margin: 28px 0 8px;
    font-size: 16px;
  }
  .note {
    font-size: 12px;
    line-height: 1.6;
    margin: 0;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 40px;
  }
  .check input {
    width: 18px;
    height: 18px;
    min-height: 18px;
    flex-shrink: 0;
  }
  .color-row {
    display: grid;
    grid-template-columns: 22px minmax(100px, 1fr) minmax(100px, 160px) auto auto;
    align-items: center;
    gap: 16px;
    padding: 12px 0;
  }
  .row-feedback {
    grid-column: 2 / -1;
    margin: 0;
    font-size: 12px;
  }
  .rgba-editor,
  .add-color {
    border-top: 0;
    padding-top: 0;
  }
  .rgba-editor > summary {
    cursor: pointer;
    color: var(--color-muted);
    font-size: 12px;
  }
  .rgba-editor label {
    margin-top: 8px;
  }
  .color-row label {
    font-size: 12px;
  }
  .color-row label small {
    display: inline;
  }
  .swatch {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 1px solid var(--color-border);
  }
  .add-color {
    margin-top: 16px;
  }
  .add-color > summary {
    cursor: pointer;
    padding: 10px 0;
  }
  .add-color form {
    margin-top: 12px;
  }
  .add-material > summary {
    grid-template-columns: 1fr auto;
    color: var(--color-accent);
  }
  @media (max-width: 700px) {
    .material > summary {
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 10px;
    }
    .material > summary > span:first-child {
      grid-column: 1 / -1;
    }
    .material > summary .material-price {
      grid-column: 2;
      grid-row: 2;
    }
    .color-row {
      grid-template-columns: 22px minmax(0, 1fr);
      gap: 10px;
    }
    .rgba-editor {
      grid-column: 2;
    }
    .color-row > .check {
      grid-column: 2;
    }
    .color-row > :global(button) {
      grid-column: 2;
      justify-self: start;
    }
  }
</style>
