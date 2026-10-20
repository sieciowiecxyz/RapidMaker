<script lang="ts" module>
  import { tv, type VariantProps } from 'tailwind-variants';
  // Adapted from shadcn-svelte Badge (MIT), using the Rapidmaker theme.
  const badgeVariants = tv({
    base: 'inline-flex w-fit shrink-0 items-center gap-2 whitespace-nowrap rounded-full border px-3 py-1 text-xs font-medium',
    variants: {
      variant: {
        default: 'border-transparent bg-brand text-white',
        secondary: 'border-highlight/15 bg-highlight/5 text-highlight',
        outline: 'border-highlight/15 text-ink',
      },
    },
    defaultVariants: { variant: 'secondary' },
  });
</script>

<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements';
  import { clsx } from 'clsx';
  import { twMerge } from 'tailwind-merge';
  let {
    children,
    class: className,
    variant,
    ...rest
  }: HTMLAttributes<HTMLSpanElement> & VariantProps<typeof badgeVariants> = $props();
</script>

<span class={twMerge(clsx(badgeVariants({ variant }), className))} {...rest}
  >{@render children?.()}</span
>
