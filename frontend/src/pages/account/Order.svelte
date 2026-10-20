<script lang="ts">
  import OrderDetail from '../../components/OrderDetail.svelte';
  let { id }: { id: string } = $props();
</script>

<OrderDetail {id} />
