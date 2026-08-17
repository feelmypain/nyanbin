<script lang="ts">
	import { API, NyanbinError, validateShortCode } from 'nyanbin/shared'
	import { onMount } from 'svelte'
	import { goto } from '$app/navigation'
	import { t } from 'svelte-intl-precompile'
	import Button from '$lib/ui/Button.svelte'
	import Loader from '$lib/ui/Loader.svelte'
	import type { PageData } from './$types'
	interface Props { data: PageData }
	let { data }: Props = $props()
	let phase = $state<'resolving' | 'missing' | 'invalid' | 'error'>('resolving')

	async function resolve() {
		phase = 'resolving'
		try {
			validateShortCode(data.code)
		} catch {
			phase = 'invalid'
			return
		}
		try {
			const { id } = await API.resolveShort(data.code)
			// Short links are always bare; the note page prompts for the password. Any fragment is dropped deliberately.
			await goto(`/note/${id}`, { replaceState: true })
		} catch (cause) {
			phase = cause instanceof NyanbinError && (cause.code === 'note_not_found' || cause.status === 404) ? 'missing' : 'error'
			requestAnimationFrame(() => document.querySelector<HTMLElement>('#short-error-title')?.focus())
		}
	}

	onMount(resolve)
</script>
<svelte:head><title>{$t('short.page_title')} — Nyanbin</title></svelte:head>
{#if phase === 'resolving'}<div class="center" role="status" aria-live="polite"><Loader/> {$t('short.resolving')}</div>
{:else if phase === 'missing'}<section class="empty panel"><span class="sleeping" aria-hidden="true">ᓚᘏᗢ ᶻ 𝗓 𐰁</span><h1>{$t('short.missing_title')}</h1><p>{$t('short.missing_body')}</p><a href="/">{$t('show.create_new')}</a></section>
{:else if phase === 'invalid'}<section class="empty panel"><h1>{$t('short.invalid_title')}</h1><p>{$t('short.invalid_body')}</p><a href="/">{$t('show.create_new')}</a></section>
{:else}<section class="empty panel" aria-labelledby="short-error-title"><h1 id="short-error-title" tabindex="-1">{$t('short.error_title')}</h1><p class="notice error" role="alert">{$t('short.error_body')}</p><Button type="button" onclick={resolve}>{$t('common.retry')}</Button><a href="/">{$t('show.create_new')}</a></section>{/if}
<style>.center { display: flex; min-height: 18rem; align-items: center; justify-content: center; gap: var(--space-2); }.empty { display: grid; gap: var(--space-4); max-width: 42rem; margin-inline: auto; padding: var(--space-5); text-align: center; }.sleeping { color: var(--accent-600); font: 700 var(--text-xl)/1 var(--font-mono); }.empty a { justify-self: center; }.empty :global(button) { justify-self: center; }</style>
