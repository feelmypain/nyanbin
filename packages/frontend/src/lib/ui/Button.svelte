<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements'
	interface Props { children?: import('svelte').Snippet; variant?: 'primary' | 'secondary' | 'danger' }
	let { children, variant = 'secondary', ...rest }: HTMLButtonAttributes & Props = $props()
</script>
<button class:primary={variant === 'primary'} class:danger={variant === 'danger'} {...rest}>{@render children?.()}</button>
