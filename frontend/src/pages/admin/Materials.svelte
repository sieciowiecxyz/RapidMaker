<script lang="ts">
  import { onMount } from 'svelte';
  import { getMaterials, updateMaterial } from '../../lib/api';
  import type { Material } from '../../lib/types';
  import Button from '../../components/ui/Button.svelte';
  let materials = $state<Material[]>([]);
  let loading = $state(true);
  let error = $state('');
  let success = $state('');
  let feedbackId = $state('');
  let busy = $state('');
  let search = $state('');
  let filtered = $derived(
    materials.filter((material) =>
      material.name.toLowerCase().includes(search.trim().toLowerCase()),
    ),
  );
  async function load() {
    loading = true;
    error = '';
    feedbackId = '';
    try {
      materials = await getMaterials();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      loading = false;
    }
  }
  onMount(load);
  async function save(event: SubmitEvent, material: Material) {
    event.preventDefault();
    busy = material.id;
    feedbackId = material.id;
    error = '';
    success = '';
    try {
      const saved = (await updateMaterial(material.id, material)).find(
        (item) => item.id === material.id,
      );
      if (saved) materials = materials.map((item) => (item.id === saved.id ? saved : item));
      success = `Zapisano ${material.name}.`;
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = '';
    }
  }
</script>

<div class="page stack">
  <div class="page-heading">
    <div>
      <p class="eyebrow">Panel zespołu</p>
      <h1>Materiały i ceny</h1>
      <p class="muted">Materiały Bambu, dostępne kolory i zgodność z drukarkami.</p>
    </div>
  </div>
  <label class="search"
    >Znajdź materiał<input bind:value={search} type="search" placeholder="PLA, PETG, ABS…" /></label
  >
  {#if error && !feedbackId}<p class="alert error" role="alert">{error}</p>{/if}
  {#if loading}<div class="panel" role="status">
      Ładowanie materiałów…
    </div>{:else if error && !materials.length}<Button onclick={load}>Spróbuj ponownie</Button
    >{:else if !filtered.length}<div class="panel empty-state">
      <h2>{search.trim() ? 'Nie znaleziono materiału' : 'Katalog jest pusty'}</h2>
      <p class="muted">
        {search.trim()
          ? 'Spróbuj innej nazwy, np. PLA lub PETG.'
          : 'Nie ma jeszcze materiałów do wyświetlenia.'}
      </p>
      {#if search}<button class="button secondary" onclick={() => (search = '')}
          >Wyczyść wyszukiwanie</button
        >{/if}
    </div>{:else}<div class="materials-list">
      {#each filtered as material (material.id)}<details class="material" name="material-editor">
          <summary>
            <span class="material-name"
              ><strong>{material.name}</strong><small>{material.description}</small></span
            >
            <span class="availability"
              >Kolory: {material.colors.filter((color) => color.inStock).length} / {material.colors
                .length}</span
            >
            <span class="compatibility" class:compatible={material.compatible}
              >{material.compatible ? 'Zgodny z drukarkami' : 'Niezgodny z drukarkami'}</span
            >
            <span class="edit">Edytuj</span>
          </summary>
          <form class="material-editor" onsubmit={(event) => save(event, material)}>
            <div class="stack">
              <label class="check"
                ><input type="checkbox" bind:checked={material.compatible} />Zgodny z naszymi
                drukarkami</label
              >
              <div class="form-grid">
                <label
                  >Filament (zł / kg)<input
                    type="number"
                    min="0.01"
                    step="0.01"
                    bind:value={material.costPerKg}
                    required
                  /></label
                >
                <label
                  >Mnożnik ceny<input
                    type="number"
                    min="0.01"
                    step="0.01"
                    bind:value={material.priceMultiplier}
                    required
                  /></label
                >
              </div>
              <small>Cennik roboczy. Zapis nie zmienia obecnej taryfy wyceny online.</small>
            </div>
            <fieldset>
              <legend>Kolory w magazynie</legend>{#each material.colors as color}<label
                  class="check color"
                >
                  <input type="checkbox" bind:checked={color.inStock} /><span
                    class="swatch"
                    style:background={color.hex}
                  ></span><span>{color.name}</span><small
                    >{color.inStock ? 'Dostępny' : 'Brak'}</small
                  >
                </label>{/each}
            </fieldset>
            <div class="material-actions">
              <Button type="submit" disabled={!!busy}
                >{busy === material.id ? 'Zapisywanie…' : 'Zapisz materiał'}</Button
              >
              {#if feedbackId === material.id}
                {#if error}<p class="alert error" role="alert">{error}</p>{:else if success}<p
                    class="saved"
                    role="status"
                  >
                    {success}
                  </p>{/if}
              {/if}
            </div>
          </form>
        </details>{/each}
    </div>{/if}
</div>

<style>
  .search {
    max-width: 420px;
  }
  .material {
    border: 0;
    border-bottom: 1px solid var(--color-border);
    padding: 0;
  }
  .material > summary {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 120px 160px 76px;
    align-items: center;
    gap: 24px;
    padding: 22px 0;
  }
  .material-name {
    min-width: 0;
  }
  .material-name strong {
    display: block;
    font-size: 16px;
    font-weight: 500;
  }
  .material-name small {
    display: block;
    margin-top: 6px;
  }
  .availability,
  .compatibility {
    font-size: 12px;
    color: var(--color-muted);
  }
  .compatible,
  .edit {
    color: var(--color-accent);
  }
  .edit {
    font-size: 13px;
    text-align: right;
    white-space: nowrap;
  }
  .edit::after {
    content: '+';
    margin-left: 10px;
  }
  .material[open] .edit::after {
    content: '−';
  }
  .material-editor {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 40px;
    padding: 8px 0 28px;
  }
  .material-actions {
    grid-column: 1 / -1;
  }
  .saved {
    color: var(--color-accent);
    margin: 12px 0 0;
  }
  .check {
    display: flex;
    gap: 10px;
    align-items: center;
    min-height: 44px;
  }
  .check input {
    width: 20px;
    height: 20px;
    min-height: 20px;
  }
  .swatch {
    height: 20px;
    width: 20px;
    border: 1px solid var(--color-border);
    border-radius: 50%;
    flex-shrink: 0;
  }
  .color small {
    margin-left: auto;
  }
  fieldset {
    border: 0;
    padding: 0;
    margin: 0;
    min-width: 0;
  }
  legend {
    padding: 0;
    margin-bottom: 8px;
    color: var(--color-muted);
    font-size: 13px;
  }
  @media (max-width: 760px) {
    .material > summary {
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 10px 16px;
    }
    .material-name {
      grid-column: 1 / -1;
    }
    .compatibility {
      grid-column: 1;
    }
    .edit {
      grid-column: 2;
      grid-row: 2 / 4;
      align-self: center;
    }
    .material-editor {
      grid-template-columns: 1fr;
      gap: 24px;
    }
  }
</style>
