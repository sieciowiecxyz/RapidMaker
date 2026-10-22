<script lang="ts">
  import { onMount, type Component } from 'svelte';
  let Page = $state<Component | null>(null);
  let error = $state(false);
  onMount(() => {
    const storefront = location.pathname === '/' || location.pathname.replace(/\/$/, '') === '/wycena';
    const load = storefront ? import('./storefront/Storefront.svelte') : import('./Workspace.svelte');
    load.then(module => { Page = module.default; }).catch(() => { error = true; });
  });
</script>

{#if Page}<Page />
{:else if error}<p role="alert">Nie udało się otworzyć strony. <a href="/">Spróbuj ponownie</a>.</p>
{:else}<p role="status">Wczytywanie…</p>{/if}
