<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements'
	interface Props { label: string; value?: boolean; help?: string }
	let { label, value = $bindable(false), help, id, ...rest }: HTMLInputAttributes & Props = $props()
	let inputId = $derived(id ?? 'switch-input')
</script>
<label for={inputId}>
	<input id={inputId} type="checkbox" bind:checked={value} {...rest} />
	<span class="track" aria-hidden="true"><span></span></span>
	<span class="copy"><strong>{label}</strong>{#if help}<small>{help}</small>{/if}</span>
</label>
<style>
	label { display: grid; grid-template-columns: auto auto 1fr; align-items: start; gap: var(--space-2); cursor: pointer; }
	input { position: absolute; width: 1px; height: 1px; opacity: 0; }
	.track { display: flex; width: 3.25rem; height: 1.75rem; padding: .18rem; border: 1px solid var(--border-strong); border-radius: 99rem; background: var(--surface-blue); transition: background var(--duration-ui) var(--ease-out); }
	.track span { display: block; width: 1.25rem; height: 1.25rem; border-radius: 50%; background: var(--ink-muted); }
	input:checked + .track { background: var(--blue-600); }
	input:checked + .track span { margin-inline-start: auto; background: var(--on-primary); }
	input:focus-visible + .track { box-shadow: var(--focus-ring); }
	.copy { display: block; }
	.copy strong, .copy small { display: block; }
	.copy small { color: var(--ink-muted); }
</style>
