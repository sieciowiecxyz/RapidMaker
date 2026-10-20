<script lang="ts">
  let {
    label,
    value = '',
    placeholder = '',
  }: { label: string; value?: string; placeholder?: string } = $props();
</script>

<label class="field-label">{label}<input bind:value {placeholder} /></label>
