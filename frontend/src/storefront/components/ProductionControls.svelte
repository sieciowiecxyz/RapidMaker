<script lang="ts">
  import type { Snippet } from 'svelte';
  import { IconCheck, IconArrowUpRight } from '@tabler/icons-svelte';
  import RadialPalette from './RadialPalette.svelte';
  import { softValue } from '../lib/motion';
  import type { Material } from '../lib/types';
  let {
    materials,
    preview,
    material = $bindable(),
    color = $bindable(),
    customShade = $bindable(),
    quality = $bindable(),
    strength = $bindable(),
  }: {
    materials: Material[];
    preview?: Snippet;
    material: string;
    color: string;
    customShade: string;
    quality: string;
    strength: string;
  } = $props();
  const selected = $derived(materials.find((item) => item.id === material));
  const colorMaterial = $derived(selected);
</script>

<div class="production-controls">
  {#if preview}<div class="mobile-preview">{@render preview()}</div>{/if}
  <fieldset class="material-field">
    <legend class="control-legend">Materiał <span>Wybierz zastosowanie</span></legend>
    <select class="material-select" aria-label="Materiał" bind:value={material}>
      <option value="" disabled>Wybierz materiał</option>
      {#each materials as item}<option value={item.id}>{item.name}</option>{/each}
    </select>
    <p class="material-description" id="material-description">
      <IconArrowUpRight size={16} stroke={1.5} /><span
        >{selected?.description ??
          (materials.length ? 'Wybierz materiał dla tego modelu.' : 'Ładowanie materiałów…')}</span
      >
    </p>
  </fieldset>

  <fieldset class="color-field">
    <legend class="control-legend"
      >Kolor <span
        class="t-text-soft"
        use:softValue={colorMaterial?.colors.find((item) => item.id === color)?.name ?? ''}
      ></span></legend
    >
    <div class="pigment-track" style:grid-template-columns="repeat(auto-fit,minmax(70px,1fr))">
      {#each colorMaterial?.colors ?? [] as item}
        <label
          class="pigment"
          class:unavailable={!item.available}
          class:selected={color === item.id}
          style={`--pigment:${item.hex}`}
        >
          <input
            class="choice-input"
            type="radio"
            name="color"
            value={item.id}
            bind:group={color}
            aria-label={item.name + (item.available ? '' : ' — dostępność do potwierdzenia')}
          />
          <span class="pigment-sample" class:custom={item.id === 'custom'} aria-hidden="true"
            ><IconCheck size={16} stroke={2} /></span
          >
          <span class="pigment-name"
            >{item.name}{#if !item.available}<small>Na zapytanie</small>{/if}</span
          >
        </label>
      {/each}
    </div>
    {#if material === 'pa'}<p class="color-hint">
        PA-CF jest dostępny wyłącznie w kolorze czarnym.
      </p>
    {:else if color === 'custom'}
      <RadialPalette bind:value={customShade} />
    {/if}
  </fieldset>

  <fieldset class="quality-field">
    <legend class="control-legend">Jakość <span>Porównaj warstwy</span></legend>
    <div class="quality-switch">
      <span
        class="quality-indicator"
        style:opacity={quality ? 1 : 0}
        style:transform={`translateX(${quality === 'fine' ? 100 : 0}%)`}
        aria-hidden="true"
      ></span>
      {#each [{ id: 'standard', name: 'Standard', detail: 'Codzienne zastosowania' }, { id: 'fine', name: 'Dokładny', detail: 'Drobniejsze detale' }] as item}
        <label class="quality-choice" class:selected={quality === item.id}>
          <input
            class="choice-input"
            type="radio"
            name="quality"
            value={item.id}
            bind:group={quality}
          />
          <span class="layer-sample" class:fine={item.id === 'fine'} aria-hidden="true"
            >{#each Array(item.id === 'fine' ? 9 : 5) as _}<i></i>{/each}</span
          >
          <span><strong>{item.name}</strong><small>{item.detail}</small></span>
        </label>
      {/each}
    </div>
  </fieldset>
  <fieldset class="strength-field">
    <legend class="control-legend">Wytrzymałość <span>Wariant konstrukcji</span></legend>
    <div class="strength-switch">
      <span
        class="strength-indicator"
        style:opacity={strength ? 1 : 0}
        aria-hidden="true"
        style:transform={`translateX(${['light', 'standard', 'reinforced'].indexOf(strength) * 100}%)`}
      ></span>
      {#each [{ id: 'light', name: 'Lekka', detail: 'Mniej materiału', cells: 3 }, { id: 'standard', name: 'Standard', detail: 'Zrównoważona', cells: 6 }, { id: 'reinforced', name: 'Wzmocniona', detail: 'Więcej materiału', cells: 9 }] as item}
        <label class="strength-choice" class:selected={strength === item.id}>
          <input
            class="choice-input"
            type="radio"
            name="strength"
            value={item.id}
            bind:group={strength}
            aria-describedby="strength-hint"
          />
          <span class="core-sample" aria-hidden="true"
            >{#each Array(9) as _, i}<i class:filled={i < item.cells}></i>{/each}</span
          >
          <strong>{item.name}</strong><small>{item.detail}</small>
        </label>
      {/each}
    </div>
    <p class="strength-hint" id="strength-hint">
      Wytrzymałość zależy też od geometrii, materiału i kierunku druku. Wariant potwierdzimy przy
      wycenie.
    </p>
  </fieldset>
</div>

<style>
  .material-select {
    width: 100%;
    padding: 16px;
    background: var(--surface);
    color: var(--ink);
    border: 1px solid var(--control-line);
    font: inherit;
  }
  .pigment-name small {
    display: block;
    font-size: 10px;
  }
</style>
