<script lang="ts">
  import { onMount } from 'svelte';
  import Badge from './ui/Badge.svelte';
  import { getDemoScenario, setDemoScenario, resetDemo } from '../lib/api';
  import type { DemoScenario } from '../lib/types';
  let { onChange }: { onChange: () => void } = $props();
  let scenario = $state<DemoScenario>('normal');
  let message = $state('');
  onMount(async () => {
    scenario = await getDemoScenario();
  });
  async function change() {
    await setDemoScenario(scenario);
    message = '';
    onChange();
  }
  async function reset() {
    await resetDemo();
    scenario = 'normal';
    message = 'Przywrócono przykładowe dane i wylogowano konto.';
    onChange();
  }
</script>

<details class="demo-tools">
  <summary
    ><Badge variant="secondary">Demo lokalne</Badge><span>Scenariusze interfejsu</span></summary
  >
  <div class="demo-content">
    <p>
      Scenariusze zmieniają dane paneli w tej przeglądarce. Wycena i płatności korzystają z
      testowych API Stripe i Furgonetki.
    </p>
    <label
      >Stan danych <select bind:value={scenario} onchange={change}
        ><option value="normal">Przykładowe dane</option><option value="empty">Puste listy</option
        ><option value="error">Błąd odczytu</option></select
      ></label
    >
    <button class="button secondary" onclick={reset}>Przywróć demo</button>
  </div>
  {#if message}<p role="status">{message}</p>{/if}
</details>
