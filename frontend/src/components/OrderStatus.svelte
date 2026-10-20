<script lang="ts">
  import type { OrderStatus } from '../lib/types';
  import { orderStatus } from '../lib/format';
  let { status }: { status: OrderStatus } = $props();
</script>

<span
  class="badge"
  class:done={['completed', 'shipped', 'ready'].includes(status)}
  class:attention={['review', 'awaiting_payment'].includes(status)}>{orderStatus[status]}</span
>

<style>
  .done {
    color: var(--color-accent);
  }
  .attention {
    color: #f1d49b;
  }
</style>
