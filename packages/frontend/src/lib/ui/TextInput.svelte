<script lang="ts">
	import { t } from 'svelte-intl-precompile'
	import type { HTMLInputAttributes } from 'svelte/elements'
	interface Props { label: string; value?: string | number | null; help?: string; error?: string; reveal?: boolean }
	let { label, value = $bindable(), help, error, reveal = false, id, type = 'text', ...rest }: HTMLInputAttributes & Props = $props()
	let visible = $state(false)
	let inputId = $derived(id ?? 'text-input')
	let resolvedType = $derived(type === 'password' && visible ? 'text' : type)
</script>
<label for={inputId}>
	<span>{label}</span>
	<div class="control">
		<input id={inputId} bind:value type={resolvedType} aria-invalid={error ? 'true' : undefined} aria-describedby={help || error ? `${inputId}-help` : undefined} {...rest} />
		{#if reveal && type === 'password'}
			<button type="button" class="reveal" onclick={() => (visible = !visible)} aria-label={visible ? $t('common.hide_password') : $t('common.show_password')}>{visible ? $t('common.hide') : $t('common.show')}</button>
		{/if}
	</div>
	{#if help || error}<small id={`${inputId}-help`} class:error>{error ?? help}</small>{/if}
</label>
<style>
	label { display: block; }
	.control { position: relative; }
	.control input { padding-inline-end: 5.5rem; }
	.reveal { position: absolute; inset-block: .25rem; inset-inline-end: .25rem; min-height: 2.5rem; padding-inline: .75rem; box-shadow: none; }
	small { display: block; margin-top: var(--space-1); color: var(--ink-muted); }
	small.error { color: var(--danger); }
</style>
