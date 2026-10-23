<script lang="ts">
  import { onMount } from 'svelte';
  import { operator, hundredths, type Rates } from '../../lib/operator';
  import Button from '../../components/ui/Button.svelte';

  type Field = Exclude<keyof Rates, 'version'>;
  const fields: {
    key: Field;
    label: string;
    unit: string;
    hint: string;
    percent?: boolean;
    integer?: boolean;
  }[] = [
    {
      key: 'setup_grosze',
      label: 'Przygotowanie zamówienia',
      unit: 'zł netto',
      hint: 'Jednorazowo za zamówienie.',
    },
    {
      key: 'handling_per_plate_grosze',
      label: 'Obsługa płyty',
      unit: 'zł netto / płytę',
      hint: 'Przygotowanie i zdjęcie wydruków z każdej płyty.',
    },
    {
      key: 'machine_per_hour_grosze',
      label: 'Praca drukarki',
      unit: 'zł netto / godzinę',
      hint: 'Amortyzacja i eksploatacja, bez energii i filamentu.',
    },
    {
      key: 'energy_per_kwh_grosze',
      label: 'Energia',
      unit: 'zł netto / kWh',
      hint: 'Cena zakupu energii.',
    },
    {
      key: 'printer_watts',
      label: 'Średnia moc drukarki',
      unit: 'W',
      hint: 'Średni pobór podczas druku, nie moc maksymalna.',
      integer: true,
    },
    {
      key: 'loss_bps',
      label: 'Rezerwa na straty',
      unit: '%',
      hint: 'Narzut na nieudane wydruki i odpady.',
      percent: true,
    },
    {
      key: 'margin_bps',
      label: 'Marża',
      unit: '%',
      hint: 'Udział zysku w cenie netto, więcej niż 0 i mniej niż 100%.',
      percent: true,
    },
    {
      key: 'minimum_net_grosze',
      label: 'Minimalna cena zamówienia',
      unit: 'zł netto',
      hint: 'Dolna granica ceny wydruków.',
    },
    {
      key: 'vat_bps',
      label: 'VAT',
      unit: '%',
      hint: 'Stawka doliczana do ceny netto.',
      percent: true,
    },
  ];
  let values = $state<Record<Field, string>>(
    Object.fromEntries(fields.map((f) => [f.key, ''])) as Record<Field, string>,
  );
  let version = $state('');
  let savedVersion = $state('');
  let configured = $state(false);
  let loading = $state(true);
  let busy = $state(false);
  let feedback = $state('');
  let failed = $state(false);
  let loadFailed = $state(false);

  async function load() {
    loading = true;
    feedback = '';
    loadFailed = false;
    try {
      const { rates } = await operator<{ rates: Rates | null }>('pricing');
      configured = rates !== null;
      if (rates) {
        version = savedVersion = rates.version;
        values = Object.fromEntries(
          fields.map((f) => [
            f.key,
            f.integer ? String(rates[f.key]) : (rates[f.key] / 100).toFixed(2),
          ]),
        ) as Record<Field, string>;
      }
    } catch (e) {
      feedback = (e as Error).message;
      failed = loadFailed = true;
    } finally {
      loading = false;
    }
  }
  onMount(load);
  async function save(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    feedback = '';
    failed = false;
    try {
      const nextVersion = version.trim();
      if (!nextVersion || new TextEncoder().encode(nextVersion).length > 128)
        throw new Error('Podaj wersję cennika o długości do 128 bajtów.');
      const result: Record<string, string | number> = { version: nextVersion };
      for (const field of fields) {
        if (field.integer) {
          if (!/^\d+$/.test(values[field.key].trim()))
            throw new Error(`${field.label}: podaj całkowitą liczbę watów.`);
          const value = Number(values[field.key]);
          if (!Number.isSafeInteger(value) || value > 10_000)
            throw new Error('Moc drukarki: podaj od 0 do 10 000 W.');
          result[field.key] = value;
        } else {
          const value = hundredths(
            values[field.key],
            field.label,
            field.percent ? 10_000 : undefined,
          );
          if (field.key === 'margin_bps' && (value <= 0 || value >= 10_000))
            throw new Error('Marża musi wynosić więcej niż 0 i mniej niż 100%.');
          result[field.key] = value;
        }
      }
      await operator('pricing', result);
      savedVersion = version = nextVersion;
      configured = true;
      feedback = `Zapisano cennik ${nextVersion}. Nowe wyceny użyją tych stawek.`;
    } catch (e) {
      feedback = (e as Error).message;
      failed = true;
    } finally {
      busy = false;
    }
  }
</script>

<div class="page stack">
  <div class="page-heading">
    <div>
      <p class="eyebrow">Panel zespołu</p>
      <h1>Ustawienia wyceny</h1>
      <p class="muted">
        Koszt filamentu pochodzi z katalogu. Czas, zużycie i liczbę płyt mierzy slicer.
      </p>
    </div>
  </div>
  {#if loading}<p role="status">Ładowanie cennika…</p>
  {:else if loadFailed}<p class="alert error" role="alert">{feedback}</p>
    <Button onclick={load}>Spróbuj ponownie</Button>
  {:else}
    {#if !configured}<p class="alert" role="status">
        Brak cennika. Uzupełnij rzeczywiste stawki, aby uruchomić automatyczną wycenę. Wpisz 0 tylko
        przy kosztach, których świadomie nie naliczasz.
      </p>{/if}
    <form class="panel settings-list" onsubmit={save}>
      <div class="setting-row">
        <label for="rates-version"
          ><strong>Wersja cennika</strong><small
            >Każda zmiana stawek wymaga nowej wersji.{savedVersion
              ? ` Obecna: ${savedVersion}.`
              : ''}</small
          ></label
        >
        <input
          id="rates-version"
          bind:value={version}
          required
          maxlength="128"
          placeholder="np. 2026-10-23.1"
        />
      </div>
      {#each fields as field}
        <div class="setting-row">
          <label for={field.key}><strong>{field.label}</strong><small>{field.hint}</small></label>
          <div class="value">
            <input
              id={field.key}
              bind:value={values[field.key]}
              inputmode={field.integer ? 'numeric' : 'decimal'}
              required
              aria-label={`${field.label} (${field.unit})`}
            /><span>{field.unit}</span>
          </div>
        </div>
      {/each}
      <div class="actions">
        <Button type="submit" disabled={busy}>{busy ? 'Zapisywanie…' : 'Zapisz cennik'}</Button
        ><small class="muted">Zapis nie zmienia wcześniej wystawionych wycen.</small>
      </div>
      {#if feedback}<p class:error={failed} role="status">{feedback}</p>{/if}
    </form>
  {/if}
</div>

<style>
  .settings-list {
    padding: 8px 24px 24px;
  }
  .setting-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 240px;
    align-items: center;
    gap: 24px;
    padding: 16px 0;
  }
  .setting-row + .setting-row {
    border-top: 1px solid var(--color-border);
  }
  .setting-row label strong {
    font-size: 14px;
    font-weight: 500;
  }
  .setting-row label small {
    display: block;
    font-size: 12px;
    color: var(--color-muted);
    margin-top: 6px;
    line-height: 1.5;
  }
  .value {
    display: grid;
    grid-template-columns: 115px minmax(0, 1fr);
    align-items: center;
    gap: 12px;
  }
  .value span {
    font-size: 12px;
    color: var(--color-muted);
  }
  .value input {
    text-align: right;
    min-height: 42px;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 20px;
    margin-top: 20px;
  }
  @media (max-width: 650px) {
    .setting-row {
      grid-template-columns: 1fr;
      gap: 12px;
    }
    .value {
      grid-template-columns: minmax(0, 1fr) 120px;
    }
    .actions {
      flex-wrap: wrap;
      gap: 12px;
    }
  }
</style>
