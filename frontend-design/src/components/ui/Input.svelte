<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';
  let { value = $bindable(), class: className = '', ...rest }: HTMLInputAttributes = $props();
</script>
<input class={`input ${className}`} bind:value {...rest} />
