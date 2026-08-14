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
	.track { position: relative; display: block; flex: 0 0 auto; width: 3.25rem; height: 1.75rem; border: 1px solid var(--border-strong); border-radius: 99rem; background: var(--surface-blue); transition: background var(--duration-ui) var(--ease-out), border-color var(--duration-ui) var(--ease-out); }
	.track span { position: absolute; inset-block-start: 50%; inset-inline-start: .18rem; width: 1.25rem; height: 1.25rem; border-radius: 50%; background: var(--ink-muted); transform: translateY(-50%); transition: transform var(--duration-ui) var(--ease-out), background var(--duration-ui) var(--ease-out); }
	label:hover .track { border-color: var(--blue-600); }
	input:checked + .track { border-color: var(--blue-700); background: var(--blue-600); }
	input:checked + .track span { background: var(--on-primary); transform: translateY(-50%) translateX(1.5rem); }
	:global([dir='rtl']) input:checked + .track span { transform: translateY(-50%) translateX(-1.5rem); }
	input:focus-visible + .track { box-shadow: var(--focus-ring); }
	.copy { display: block; }
	.copy strong, .copy small { display: block; }
	.copy small { color: var(--ink-muted); }
</style>
