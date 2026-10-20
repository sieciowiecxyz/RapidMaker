<script lang="ts">
  import { money, parcels } from '../lib/format';

  type Packing = {
    status: 'estimated' | 'manual_review';
    parcels: {
      box_id: string;
      outer_mm: number[];
      weight_g: number;
      placements?: { part_id: string; position_mm: number[]; dimensions_mm: number[] }[];
    }[];
    packaging_grosze: number;
    reason?: string | null;
    geometry_source?: string;
  };

  let {
    packing,
    models,
    delivery = 'courier',
  }: {
    packing: Packing;
    models: { id: string; name: string }[];
    delivery?: 'courier' | 'locker' | 'pickup';
  } = $props();

  const number = (value: number) =>
    new Intl.NumberFormat('pl-PL', { maximumFractionDigits: 2 }).format(value);
  const dimensions = (values: number[]) => values.map((value) => number(value / 10)).join(' × ');
  function contents(placements: Packing['parcels'][number]['placements']) {
    const counts = new Map<string, number>();
    for (const part of placements ?? []) {
      counts.set(part.part_id, (counts.get(part.part_id) ?? 0) + 1);
    }
    return [...counts].map(([id, quantity]) => ({
      id,
      name: models.find((model) => model.id === id)?.name ?? 'Część wydruku',
      quantity,
    }));
  }
  let totalWeight = $derived(packing.parcels.reduce((sum, parcel) => sum + parcel.weight_g, 0));
</script>

<section class="parcel-plan" aria-labelledby="parcel-plan-title">
  <div class="heading">
    <h3 id="parcel-plan-title">
      {delivery === 'pickup' ? 'Odbiór osobisty' : 'Szacowany plan pakowania'}
    </h3>
    {#if delivery !== 'pickup' && packing.parcels.length}
      <span class="muted">{parcels(packing.parcels.length)} · {number(totalWeight / 1000)} kg</span>
    {/if}
  </div>
  {#if delivery === 'pickup'}
    <p class="muted">Odbierzesz wszystkie części razem. Bez kosztów dostawy.</p>
  {:else if packing.status === 'manual_review' || !packing.parcels.length}
    <p class="muted">Dopasujemy opakowania i potwierdzimy koszt dostawy przed płatnością.</p>
    <p class="next-step">
      Liczbę paczek obliczamy automatycznie z danych modeli. Duże lub nietypowe zamówienie oceni nasz zespół.
    </p>
  {:else}
    <ol>
      {#each packing.parcels as parcel, index}
        {@const parts = contents(parcel.placements)}
        <li>
          <details>
            <summary>
              <strong>Paczka {index + 1}</strong>
              <span class="measurements"
                >{dimensions(parcel.outer_mm)} cm <span class="muted">·</span>
                {number(parcel.weight_g / 1000)} kg</span
              >
            </summary>
            {#if parts.length}
              <ul aria-label={`Zawartość paczki ${index + 1}`}>
                {#each parts as part (part.id)}
                  <li>
                    <span class="part-name">{part.name}</span><span>{part.quantity} szt.</span>
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="muted">Dokładną zawartość potwierdzimy przy pakowaniu.</p>
            {/if}
          </details>
        </li>
      {/each}
    </ol>
    {#if packing.packaging_grosze > 0}
      <p class="packaging muted">Opakowania: {money(packing.packaging_grosze / 100)}</p>
    {/if}
  {/if}
</section>

<style>
  .parcel-plan {
    min-width: 0;
    font-size: 13px;
  }
  .heading {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 6px 16px;
  }
  h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }
  p {
    margin: 8px 0 0;
    line-height: 1.6;
  }
  .next-step {
    font-size: 12px;
  }
  ol,
  ul {
    margin: 10px 0 0;
    padding: 0;
    list-style: none;
  }
  .parcel-plan details {
    border: 0;
    padding: 0;
  }
  summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 16px;
    padding: 13px 0;
    cursor: pointer;
    list-style: none;
  }
  summary::-webkit-details-marker {
    display: none;
  }
  summary::after {
    content: '+';
    margin-left: auto;
    color: var(--color-accent);
  }
  details[open] > summary::after {
    content: '−';
  }
  summary:focus-visible {
    outline: 2px solid var(--color-accent);
    outline-offset: 3px;
    border-radius: 4px;
  }
  summary strong {
    font-size: 13px;
    font-weight: 500;
  }
  .measurements {
    color: var(--color-muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  ul {
    margin: 0 0 13px;
    display: grid;
    gap: 7px;
  }
  ul li {
    display: flex;
    justify-content: space-between;
    gap: 16px;
  }
  ul li > :last-child {
    flex-shrink: 0;
    color: var(--color-muted);
  }
  .part-name {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  details > p {
    margin: 0 0 13px;
  }
  .packaging {
    font-size: 12px;
  }
  @media (max-width: 420px) {
    summary {
      gap: 6px 10px;
    }
    .measurements {
      font-size: 11px;
    }
  }
</style>
