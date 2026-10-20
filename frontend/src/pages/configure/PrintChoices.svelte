<script lang="ts">
  import type { Material, PrintModel, PrintOptionsCatalog } from '../../lib/types';
  let {
    model,
    materials,
    catalog,
    busy,
    onchange,
  }: {
    model: PrintModel;
    materials: Material[];
    catalog: PrintOptionsCatalog;
    busy: boolean;
    onchange: (patch: Partial<PrintModel>) => void;
  } = $props();
  let search = $state('');
  let moreMaterials = $state(false);
  let material = $derived(materials.find((item) => item.id === model.options.materialId));
  let common = $derived(
    materials.filter((item) => /PLA Basic|PETG HF|ABS$|TPU/i.test(item.name)).slice(0, 4),
  );
  function options(patch: Partial<PrintModel['options']>) {
    onchange({ options: { ...model.options, ...patch } });
  }
  function chooseMaterial(item: Material) {
    options({ materialId: item.id, color: item.colors.find((color) => color.inStock)?.name ?? '' });
  }
</script>

<div class="print-choices">
  <fieldset>
    <legend><span>01</span> Materiał i kolor</legend>
    <h3>Materiał</h3>
    <div class="material-list">
      {#each common as item}
        <button
          class="material-option"
          class:chosen={item.id === model.options.materialId}
          aria-pressed={item.id === model.options.materialId}
          disabled={busy || !item.compatible || !item.colors.some((color) => color.inStock)}
          onclick={() => chooseMaterial(item)}
        >
          <span class="radio-dot" aria-hidden="true"></span>
          <span
            ><strong>{item.name.replace('Bambu ', '')}</strong><small>{item.description}</small
            ></span
          >
        </button>
      {/each}
    </div>
    <button
      class="catalog-toggle"
      aria-expanded={moreMaterials}
      onclick={() => (moreMaterials = !moreMaterials)}
    >
      {moreMaterials ? 'Ukryj katalog ↑' : 'Inny materiał? Zobacz cały katalog ↓'}
    </button>
    {#if moreMaterials}
      <label class="catalog-search"
        >Szukaj materiału<input bind:value={search} placeholder="Nazwa materiału…" /></label
      >
      <label
        >Cały katalog<select
          disabled={busy}
          value={model.options.materialId}
          onchange={(event) => {
            const item = materials.find((item) => item.id === event.currentTarget.value);
            if (item) chooseMaterial(item);
          }}
        >
          {#each materials.filter((item) => item.name
                .toLowerCase()
                .includes(search.toLowerCase()) || item.id === model.options.materialId) as item}
            <option
              value={item.id}
              disabled={!item.compatible || !item.colors.some((color) => color.inStock)}
              >{item.name}{!item.compatible
                ? ' · niezgodny'
                : !item.colors.some((color) => color.inStock)
                  ? ' · niedostępny'
                  : ''}</option
            >
          {/each}
        </select></label
      >
    {/if}
    {#if material && !common.some((item) => item.id === material?.id)}<p class="selected-material">
        <strong>{material.name}</strong> · {material.description}
      </p>{/if}
    <div class="color-heading">Kolor <strong>{model.options.color}</strong></div>
    <div class="swatches" role="group" aria-label="Kolor materiału">
      {#each material?.colors ?? [] as color}
        <button
          class="color-option"
          class:chosen={model.options.color === color.name}
          aria-pressed={model.options.color === color.name}
          disabled={busy || !color.inStock}
          title={`${color.name}${!color.inStock ? ' — niedostępny' : ''}`}
          onclick={() => options({ color: color.name })}
        >
          <span class="swatch" style={`--swatch:${color.hex}`} aria-hidden="true"></span>
          <span>{color.name}{!color.inStock ? ' · brak' : ''}</span>
          {#if model.options.color === color.name}<span class="check" aria-hidden="true">✓</span
            >{/if}
        </button>
      {/each}
    </div>
  </fieldset>
  <fieldset>
    <legend><span>02</span> Wygląd powierzchni</legend>
    <p class="hint">Cieńsze warstwy lepiej oddają detale, ale wydruk trwa dłużej.</p>
    <div class="choices quality-choices">
      {#each catalog.qualities as quality, index}
        <button
          class="choice"
          class:chosen={quality.value === model.options.quality}
          disabled={busy}
          aria-pressed={quality.value === model.options.quality}
          onclick={() => options({ quality: quality.value })}
        >
          <svg viewBox="0 0 88 44" aria-hidden="true"
            ><path d="M10 36V8h68v28z" fill="currentColor" opacity=".08" />
            {#each Array.from({ length: [4, 6, 10][index] }) as _, row}<path
                d={`M10 ${8 + row * (28 / [4, 6, 10][index])}h68`}
                stroke="currentColor"
                stroke-width="1.3"
              />{/each}
          </svg>
          <strong>{quality.label.split(' · ')[0]}</strong><span class="measure"
            >{quality.label.split(' · ')[1]}</span
          >
          <small>{quality.description}</small>
          {#if quality.value === 'standard'}<span class="recommended">Najczęstszy wybór</span>{/if}
        </button>
      {/each}
    </div>
    <p class="caption">Schemat warstw — nie jest wizualizacją Twojego modelu.</p>
  </fieldset>
  <fieldset>
    <legend><span>03</span> Budowa wydruku</legend>
    <p class="hint">Ściany tworzą obrys części. Wypełnienie to struktura wewnątrz.</p>
    <div class="choices strength-choices">
      {#each catalog.strengths.filter((item) => item.value !== 'custom') as strength, index}
        <button
          class="choice"
          class:chosen={strength.value === model.strength}
          disabled={busy}
          aria-pressed={strength.value === model.strength}
          onclick={() => onchange({ strength: strength.value })}
        >
          <svg viewBox="0 0 88 44" aria-hidden="true">
            <rect
              x="20"
              y="6"
              width="48"
              height="32"
              rx="2"
              fill="none"
              stroke="currentColor"
              stroke-width={[2, 3, 5][index]}
            />
            {#each Array.from({ length: [2, 4, 7][index] }) as _, row}<path
                d={`M${25 + row * (37 / [2, 4, 7][index])} 10v24`}
                stroke="currentColor"
                stroke-width="1"
                opacity=".55"
              />{/each}
            <path d="M24 17h40m-40 10h40" stroke="currentColor" stroke-width="1" opacity=".55" />
          </svg>
          <strong>{strength.label}</strong><small
            >{index === 0
              ? 'Dekoracje i modele pokazowe'
              : index === 1
                ? 'Części do codziennego użytku'
                : 'Więcej materiału w środku i obrysie'}</small
          >
          <span class="measure">{strength.wallCount} ściany · {strength.infill}%</span>
        </button>
      {/each}
    </div>
    <button
      class="catalog-toggle"
      aria-pressed={model.strength === 'custom'}
      disabled={busy}
      onclick={() => onchange({ strength: model.strength === 'custom' ? 'standard' : 'custom' })}
      >Własne ściany i wypełnienie {model.strength === 'custom' ? '↑' : '↓'}</button
    >
    {#if model.strength === 'custom'}
      <div class="form-grid custom-settings">
        <label
          >Liczba ścian<input
            type="number"
            min="2"
            max="8"
            step="1"
            disabled={busy}
            value={model.advanced.wallCount}
            onchange={(event) =>
              onchange({
                advanced: { ...model.advanced, wallCount: Number(event.currentTarget.value) },
              })}
          /></label
        >
        <label
          >Wypełnienie (%)<input
            type="number"
            min="10"
            max="100"
            step="1"
            disabled={busy}
            value={model.advanced.infill}
            onchange={(event) =>
              onchange({
                advanced: { ...model.advanced, infill: Number(event.currentTarget.value) },
              })}
          /></label
        >
        <label
          >Wzór wypełnienia<select
            disabled={busy}
            value={model.advanced.pattern}
            onchange={(event) =>
              onchange({
                advanced: {
                  ...model.advanced,
                  pattern: event.currentTarget.value as PrintModel['advanced']['pattern'],
                },
              })}
            >{#each catalog.patterns as pattern}<option value={pattern.value}
                >{pattern.label}</option
              >{/each}</select
          ></label
        >
      </div>
    {/if}
    <p class="caption">
      Wytrzymałość zależy także od materiału, kształtu i kierunku druku. Schemat jest orientacyjny.
    </p>
  </fieldset>
</div>

<style>
  .print-choices {
    display: grid;
    gap: 30px;
  }
  fieldset {
    border: 0;
    padding: 0;
    margin: 0;
    min-width: 0;
  }
  legend {
    padding: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--color-heading);
    margin-bottom: 14px;
  }
  legend > span {
    color: var(--color-muted);
    font-size: 11px;
    margin-right: 9px;
    font-weight: 400;
  }
  h3 {
    display: none;
  }
  .material-list {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }
  button {
    font: inherit;
  }
  .material-option {
    display: flex;
    gap: 10px;
    text-align: left;
    padding: 13px;
    border: 1px solid var(--color-option-border);
    background: var(--color-option);
    border-radius: 8px;
    color: var(--color-text);
    cursor: pointer;
  }
  .material-option > span:last-child {
    display: grid;
    gap: 4px;
  }
  .material-option strong {
    font-size: 13px;
    font-weight: 500;
  }
  .material-option small {
    font-size: 11px;
    color: var(--color-muted);
    line-height: 1.5;
  }
  .radio-dot {
    width: 12px;
    height: 12px;
    flex: 0 0 12px;
    border: 1px solid var(--color-muted);
    border-radius: 50%;
    margin-top: 3px;
  }
  .chosen .radio-dot {
    border: 4px solid var(--color-accent);
  }
  .chosen {
    border-color: var(--color-accent);
    background: var(--color-selected);
  }
  .material-option:hover:not(:disabled),
  .choice:hover:not(:disabled),
  .color-option:hover:not(:disabled) {
    border-color: var(--color-accent);
  }
  .color-option.chosen {
    background: var(--color-selected);
  }
  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .catalog-toggle {
    border: 0;
    background: transparent;
    color: var(--color-muted);
    font-size: 12px;
    min-height: 40px;
    padding: 8px 0;
    text-align: left;
    cursor: pointer;
  }
  .catalog-toggle:hover {
    color: var(--color-heading);
  }
  label {
    display: grid;
    gap: 6px;
    font-size: 12px;
  }
  .catalog-search {
    margin: 8px 0 12px;
  }
  .selected-material {
    font-size: 12px;
    color: var(--color-muted);
    line-height: 1.6;
  }
  .color-heading {
    display: flex;
    gap: 10px;
    margin: 16px 0 10px;
    font-size: 12px;
    color: var(--color-muted);
  }
  .color-heading strong {
    font-weight: 400;
    color: var(--color-text);
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 7px;
  }
  .color-option {
    display: flex;
    align-items: center;
    gap: 7px;
    min-height: 36px;
    padding: 5px 9px 5px 6px;
    background: var(--color-option);
    border: 1px solid var(--color-option-border);
    border-radius: 6px;
    color: var(--color-text);
    font-size: 11px;
    cursor: pointer;
  }
  .color-option.chosen {
    border-color: var(--color-accent);
  }
  .swatch {
    width: 20px;
    height: 20px;
    background: var(--swatch);
    border-radius: 50%;
    border: 1px solid #ffffff24;
  }
  .check {
    color: var(--color-accent);
  }
  .hint {
    color: var(--color-muted);
    font-size: 12px;
    margin: 0 0 13px;
    line-height: 1.6;
  }
  .choices {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }
  .choice {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    text-align: left;
    gap: 6px;
    background: var(--color-option);
    border: 1px solid var(--color-option-border);
    border-radius: 8px;
    padding: 12px;
    color: var(--color-text);
    cursor: pointer;
  }
  .choice.chosen {
    border-color: var(--color-accent);
    background: var(--color-selected);
  }
  .choice svg {
    width: 66px;
    height: 34px;
    color: var(--color-muted);
    margin-bottom: 3px;
  }
  .choice.chosen svg {
    color: var(--color-accent);
  }
  .choice strong {
    font-size: 12px;
    font-weight: 500;
  }
  .choice small {
    color: var(--color-muted);
    font-size: 11px;
    line-height: 1.5;
  }
  .measure {
    font-size: 11px;
    color: var(--color-muted);
  }
  .recommended {
    font-size: 9px;
    color: var(--color-accent);
    margin-top: auto;
    padding-top: 3px;
  }
  .caption {
    color: var(--color-muted);
    font-size: 10px;
    line-height: 1.6;
    margin: 9px 0 0;
  }
  .custom-settings {
    margin: 10px 0;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  @media (max-width: 480px) {
    .material-list {
      grid-template-columns: 1fr;
    }
    .choice {
      padding: 10px 8px;
    }
    .choice svg {
      width: 52px;
    }
    .choice strong {
      font-size: 11px;
    }
  }
</style>
