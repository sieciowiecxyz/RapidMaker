<script lang="ts">
  import type { Snippet } from 'svelte';
  let {
    children,
    onclick,
    disabled = false,
    variant = 'primary',
    class: className = '',
    type = 'button',
  }: {
    children: Snippet;
    onclick?: () => void;
    disabled?: boolean;
    variant?: 'primary' | 'secondary' | 'ghost';
    class?: string;
    type?: 'button' | 'submit';
  } = $props();
</script>

<button {type} {onclick} {disabled} class="button {variant} {className}"
  >{@render children()}</button
>
