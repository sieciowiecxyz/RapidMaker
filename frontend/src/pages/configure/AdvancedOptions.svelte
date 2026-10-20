<script lang="ts">
  import type { PrintModel, PrintOptionsCatalog, PrintSettings, Choice } from '../../lib/types';
  let {
    model,
    catalog,
    busy,
    onchange,
  }: {
    model: PrintModel;
    catalog: PrintOptionsCatalog;
    busy: boolean;
    onchange: (patch: Partial<PrintModel>) => void;
  } = $props();
  let fileError = $state('');
  let selections = $derived<
    {
      key: 'orientation' | 'supports' | 'finish' | 'fit' | 'turnaround';
      label: string;
      choices: Choice<string>[];
    }[]
  >([
    { key: 'orientation', label: 'Orientacja wydruku', choices: catalog.orientations },
    { key: 'supports', label: 'Podpory', choices: catalog.supports },
    { key: 'finish', label: 'Wykończenie', choices: catalog.finishes },
    { key: 'fit', label: 'Pasowanie części', choices: catalog.fits },
    { key: 'turnaround', label: 'Termin', choices: catalog.turnarounds },
  ]);
  function change(patch: Partial<PrintSettings>) {
    onchange({ advanced: { ...model.advanced, ...patch } });
  }
  function drawing(file?: File) {
    fileError = '';
    if (!file) return;
    if (!/\.pdf$/i.test(file.name) || !file.size || file.size > 10 * 1024 * 1024) {
      fileError = 'Wybierz rysunek PDF do 10 MB.';
      return;
    }
    change({ drawing: { name: file.name, size: file.size } });
  }
</script>

<details class="advanced-options">
  <summary>Więcej ustawień i wymagania</summary>
  <div class="stack advanced-content">
    <p class="muted intro">
      Zwykły wydruk? Możesz zostawić ustawienia domyślne. Szczególne wymagania potwierdzi zespół.
    </p>
    {#if model.format === 'STL'}<label
        >Jednostki pliku STL<select
          aria-label="Jednostki pliku STL"
          disabled={busy}
          value={model.units}
          onchange={(e) => onchange({ units: e.currentTarget.value as PrintModel['units'] })}
          ><option value="mm">Milimetry</option><option value="cm">Centymetry</option><option
            value="in">Cale</option
          ></select
        ></label
      >{/if}
    <h3>Indywidualne wymagania</h3>
    <p class="muted hint">
      Zmiana poniższych ustawień na niestandardowe kieruje zlecenie do ręcznej wyceny. Możliwość
      wykonania, materiał, dopłatę i termin potwierdzi zespół.
    </p>
    {#each selections as field}<label
        >{field.label}<select
          aria-label={field.label}
          aria-describedby={`${field.key}-hint-${model.id}`}
          disabled={busy}
          value={model.advanced[field.key]}
          onchange={(e) => change({ [field.key]: e.currentTarget.value })}
          >{#each field.choices as item}<option value={item.value}>{item.label}</option
            >{/each}</select
        ><small class="muted" id={`${field.key}-hint-${model.id}`}
          >{field.choices.find((item) => item.value === model.advanced[field.key])
            ?.description}</small
        ></label
      >{/each}
    <div class="form-grid">
      <label
        >Inserty gwintowane · liczba<input
          type="number"
          min="0"
          max="100"
          step="1"
          value={model.advanced.insertCount}
          disabled={busy}
          onchange={(e) => change({ insertCount: Number(e.currentTarget.value) })}
        /></label
      ><label
        >Rozmiar insertów<select
          aria-label="Rozmiar insertów"
          value={model.advanced.insertSize}
          disabled={busy || model.advanced.insertCount === 0}
          onchange={(e) =>
            change({ insertSize: e.currentTarget.value as PrintSettings['insertSize'] })}
          >{#each catalog.insertSizes as size}<option value={size}>{size}</option>{/each}</select
        ></label
      >
    </div>
    <label
      >Ważne wymagania<textarea
        rows="3"
        disabled={busy}
        value={model.notes}
        placeholder="Ważne wymiary, miejsca styku, kierunek obciążenia…"
        maxlength="4000"
        onchange={(e) => onchange({ notes: e.currentTarget.value })}></textarea></label
    >
    <label
      >Rysunek techniczny · PDF do 10 MB<input
        type="file"
        accept=".pdf,application/pdf"
        disabled={busy}
        onchange={(e) => {
          drawing(e.currentTarget.files?.[0]);
          e.currentTarget.value = '';
        }}
      /></label
    >
    {#if model.advanced.drawing}<div class="drawing">
        <span>{model.advanced.drawing.name}</span><button
          class="button ghost"
          disabled={busy}
          onclick={() => change({ drawing: null })}>Usuń rysunek</button
        >
      </div>{/if}
    {#if fileError}<p class="alert error" role="alert">{fileError}</p>{/if}
    <small class="muted"
      >Zapisujemy tylko nazwę i rozmiar PDF. Zawartość pliku nie jest odczytywana ani wysyłana.</small
    >
  </div>
</details>

<style>
  .advanced-options {
    border-top: 1px solid var(--color-border);
    margin-top: 4px;
  }
  summary {
    cursor: pointer;
    min-height: 44px;
    align-content: center;
    color: var(--color-muted);
    font-size: 12px;
  }
  .advanced-content {
    padding-top: 16px;
  }
  label {
    display: grid;
    gap: 8px;
    font-size: 13px;
  }
  small,
  .hint,
  .intro {
    font-size: 12px;
    line-height: 1.6;
    margin: 0;
  }
  h3 {
    font-size: 15px;
    margin: 8px 0 0;
  }
  .drawing {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    font-size: 13px;
    overflow-wrap: anywhere;
  }
  .drawing span {
    min-width: 0;
  }
  .form-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  input[type='file'] {
    max-width: 100%;
    font-size: 12px;
  }
  @media (max-width: 420px) {
    .form-grid {
      grid-template-columns: 1fr;
    }
    .drawing {
      align-items: start;
      flex-direction: column;
    }
  }
</style>
