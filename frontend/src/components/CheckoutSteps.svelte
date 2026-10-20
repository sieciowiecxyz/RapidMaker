<script lang="ts">
  let { current, onStep }: { current: number; onStep?: (step: number) => void } = $props();
  const steps = ['Druk', 'Wysyłka', 'Podsumowanie', 'Płatność'];
</script>

<nav class="checkout-steps" aria-label="Etapy zamówienia">
  <ol>
    {#each steps as label, index}
      {@const step = index + 1}
      <li
        class:complete={step < current}
        class:active={step === current}
        aria-current={step === current ? 'step' : undefined}
      >
        {#if step === 1 && current > 1}
          <a href="/wycena"><span class="number">✓</span><span>{label}</span></a>
        {:else if step < current && onStep}
          <button type="button" onclick={() => onStep?.(step)}
            ><span class="number">✓</span><span>{label}</span></button
          >
        {:else}
          <span class="step"
            ><span class="number">{step < current ? '✓' : step}</span><span>{label}</span></span
          >
        {/if}
      </li>
    {/each}
  </ol>
</nav>

<style>
  .checkout-steps {
    margin-bottom: 28px;
    padding: 0;
  }
  ol {
    display: flex;
    list-style: none;
    padding: 0;
    margin: 0;
  }
  li {
    flex: 1;
    position: relative;
    color: var(--color-muted);
  }
  li:not(:last-child)::after {
    content: '';
    position: absolute;
    top: 16px;
    left: calc(50% + 26px);
    right: calc(-50% + 26px);
    height: 1px;
    background: var(--color-border);
  }
  li.complete::after {
    background: var(--color-primary);
  }
  a,
  button,
  .step {
    display: flex;
    align-items: center;
    flex-direction: column;
    gap: 9px;
    font-size: 13px;
    color: inherit;
    text-decoration: none;
  }
  button {
    background: none;
    border: 0;
    padding: 0;
    width: 100%;
    cursor: pointer;
  }
  .number {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 1px solid var(--color-border);
    border-radius: 50%;
    font-size: 12px;
  }
  .active {
    color: var(--color-heading);
    font-weight: 600;
  }
  .active .number {
    background: var(--color-primary);
    border-color: var(--color-primary);
    color: white;
  }
  .complete {
    color: var(--color-accent);
  }
  @media (max-width: 560px) {
    a,
    button,
    .step {
      font-size: 11px;
    }
    .checkout-steps {
      margin-bottom: 24px;
    }
  }
</style>
