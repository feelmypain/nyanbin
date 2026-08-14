<script lang="ts">
	import { t } from 'svelte-intl-precompile'
	import { status } from '$lib/stores/status'
	let imprintUrl = $derived.by(() => {
		if ($status.state !== 'ready' || !$status.value.branding.imprintUrl) return ''
		try { const parsed = new URL($status.value.branding.imprintUrl); return parsed.protocol === 'https:' || parsed.protocol === 'http:' ? parsed.toString() : '' } catch { return '' }
	})
</script>
<svelte:head><title>{$t('imprint.title')} — Nyanbin</title></svelte:head>
<section class="panel"><h1>{$t('imprint.title')}</h1>{#if imprintUrl}<p>{$t('imprint.external')}</p><a href={imprintUrl} rel="noreferrer">{$t('imprint.open')}</a>{:else}<p>{$t('imprint.unavailable')}</p>{/if}</section>
<style>section { max-width: 42rem; margin-inline: auto; padding: var(--space-5); } h1 { margin-bottom: var(--space-3); }</style>
