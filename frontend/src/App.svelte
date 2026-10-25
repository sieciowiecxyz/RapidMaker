<script lang="ts">
  import { onMount, type Component } from 'svelte';
  let Page = $state<Component | null>(null);
  let error = $state(false);
  onMount(() => {
    const storefront =
      location.pathname === '/' || location.pathname.replace(/\/$/, '') === '/wycena';
    const show = (module: { default: Component }) => {
      Page = module.default;
    };
    const failed = () => {
      error = true;
    };
    // Keep imports in separate branches: Vite must preload each page's own CSS.
    if (storefront) void import('./storefront/Storefront.svelte').then(show, failed);
    else void import('./Workspace.svelte').then(show, failed);
  });
</script>

{#if Page}<Page />
{:else if error}<p role="alert">Nie udało się otworzyć strony. <a href="/">Spróbuj ponownie</a>.</p>
{:else}<p role="status">Wczytywanie…</p>{/if}
