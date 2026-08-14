<script lang="ts">
	import type { HTMLTextareaAttributes } from 'svelte/elements'
	interface Props { label: string; value?: string; help?: string }
	let { label, value = $bindable(''), help, id, ...rest }: HTMLTextareaAttributes & Props = $props()
	let textareaId = $derived(id ?? 'text-area')
</script>
<label for={textareaId}>
	<span>{label}</span>
	<textarea id={textareaId} bind:value aria-describedby={help ? `${textareaId}-help` : undefined} {...rest}></textarea>
	{#if help}<small id={`${textareaId}-help`} class="help">{help}</small>{/if}
</label>
<style>
	textarea { min-height: 18rem; padding: var(--space-4); font-family: var(--font-mono); line-height: 1.55; }
	small { display: block; margin-top: var(--space-1); }
	@media (max-width: 48rem) { textarea { min-height: 13.75rem; } }
</style>
