<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  let { children, variant = 'primary', class: className = '', ...rest }: HTMLButtonAttributes & { children?: Snippet; variant?: 'primary' | 'secondary' | 'text' } = $props();
</script>
<button class={`button button-${variant} ${className}`} {...rest}>{#if children}{@render children()}{/if}</button>
