<script lang="ts">
	import { API, NyanbinError, SECRET_BYTES, decodeBase64Url, decryptPayload, type NoteInfo, type PrivatePayload } from 'nyanbin/shared'
	import { onMount } from 'svelte'
	import { locale, t } from 'svelte-intl-precompile'
	import Button from '$lib/ui/Button.svelte'
	import Loader from '$lib/ui/Loader.svelte'
	import ShowNote from '$lib/ui/ShowNote.svelte'
	import TextInput from '$lib/ui/TextInput.svelte'
	import { formatExpiry } from '$lib/utils'
	import type { PageData } from './$types'
	interface Props { data: PageData }
	let { data }: Props = $props()
	let revealPhase = $state<'loading' | 'gate' | 'revealing' | 'decrypting' | 'shown' | 'missing' | 'invalid' | 'error'>('loading')
	let info = $state<NoteInfo | null>(null)
	let payload = $state<PrivatePayload | null>(null)
	let secret = $state<Uint8Array | null>(null)
	let password = $state('')
	let error = $state('')
	let retryMode = $state<'info' | 'reveal' | null>(null)
	let consumedFailure = $state(false)

	async function loadInfo() {
		error = ''
		retryMode = null
		consumedFailure = false
		revealPhase = 'loading'
		try {
			info = await API.info(data.id)
			revealPhase = 'gate'
		} catch (cause) {
			revealPhase = cause instanceof NyanbinError && (cause.code === 'note_not_found' || cause.status === 404) ? 'missing' : 'error'
			error = $t('show.errors.info')
			retryMode = revealPhase === 'error' ? 'info' : null
			requestAnimationFrame(() => document.querySelector<HTMLElement>('#reveal-error-title')?.focus())
		}
	}

	onMount(async () => {
		try {
			secret = decodeBase64Url(window.location.hash.slice(1), { length: SECRET_BYTES, label: 'link secret' })
		} catch {
			revealPhase = 'invalid'
			return
		}
		await loadInfo()
	})

	async function reveal(event: SubmitEvent) {
		event.preventDefault()
		if (!secret || !info) return
		error = ''
		try {
			revealPhase = 'revealing'
			const response = await API.reveal(data.id)
			revealPhase = 'decrypting'
			payload = await decryptPayload(response.envelope, { id: data.id, secret, password, lifecycle: { expiresAt: info.lifecycle.expiresAt, ...(info.lifecycle.maxReads === undefined ? {} : { maxReads: info.lifecycle.maxReads }) } })
			revealPhase = 'shown'
			requestAnimationFrame(() => document.querySelector<HTMLElement>('#note-title')?.focus())
		} catch (cause) {
			const consumedDuringDecrypt = revealPhase === 'decrypting'
			if (consumedDuringDecrypt) error = $t('show.errors.decrypt_consumed')
			else if (cause instanceof NyanbinError && (cause.code === 'note_not_found' || cause.status === 404)) error = $t('show.errors.consumed')
			else if (cause instanceof NyanbinError && cause.code === 'NETWORK_ERROR') error = $t('show.errors.network')
			else error = $t('show.errors.reveal_consumed')
			consumedFailure = consumedDuringDecrypt
			const mayHaveReads = info.lifecycle.remainingReads === undefined || info.lifecycle.remainingReads > 1
			retryMode = cause instanceof NyanbinError && (cause.code === 'note_not_found' || cause.status === 404) ? null : consumedDuringDecrypt && !mayHaveReads ? null : 'reveal'
			revealPhase = 'error'
			requestAnimationFrame(() => document.querySelector<HTMLElement>('#reveal-error-title')?.focus())
		}
	}
</script>
<svelte:head><title>{$t('show.page_title')} — Nyanbin</title></svelte:head>
{#if revealPhase === 'loading'}<div class="center" role="status" aria-live="polite"><Loader/> {$t('common.loading')}</div>
{:else if revealPhase === 'missing'}<section class="empty panel"><span class="sleeping" aria-hidden="true">ᓚᘏᗢ ᶻ 𝗓 𐰁</span><h1>{$t('show.missing_title')}</h1><p>{$t('show.missing_body')}</p><a href="/">{$t('show.create_new')}</a></section>
{:else if revealPhase === 'invalid'}<section class="empty panel"><h1>{$t('show.invalid_title')}</h1><p>{$t('show.invalid_body')}</p><a href="/">{$t('show.create_new')}</a></section>
{:else if revealPhase === 'shown' && payload}<ShowNote {payload}/>
{:else if revealPhase === 'error'}<section class="empty panel" aria-labelledby="reveal-error-title"><h1 id="reveal-error-title" tabindex="-1">{consumedFailure ? $t('show.consumed_error_title') : $t('show.error_title')}</h1><p class="notice error" role="alert">{error || $t('show.errors.info')}</p>{#if retryMode === 'info'}<Button type="button" onclick={loadInfo}>{$t('show.retry_info')}</Button>{:else if retryMode === 'reveal'}<p>{$t('show.retry_consumed_help')}</p><Button type="button" onclick={loadInfo}>{$t('show.retry_reveal')}</Button>{/if}<a href="/">{$t('show.create_new')}</a></section>
{:else}<section class="gate panel" data-testid="reveal-gate" aria-labelledby="gate-title"><header><div><span class="eyebrow">{$t('show.eyebrow')}</span><h1 id="gate-title">{$t('show.gate_title')}</h1></div><span class="cat" aria-hidden="true">/ᐠ - ˕ -マ</span></header><p class="notice warning">{$t('show.consume_warning')}</p>{#if info}<dl data-testid="note-lifecycle"><div><dt>{$t('show.expires')}</dt><dd>{formatExpiry(info.lifecycle.expiresAt, $locale)}</dd></div>{#if info.lifecycle.remainingReads !== undefined}<div><dt>{$t('show.remaining')}</dt><dd>{info.lifecycle.remainingReads}</dd></div>{/if}</dl>{/if}<form onsubmit={reveal}><TextInput id="show-note-password" data-testid="show-note-password" type="password" reveal autocomplete="current-password" label={$t('show.password_optional')} help={$t('show.password_help')} bind:value={password}/><Button data-testid="show-note-button" variant="primary" type="submit" disabled={revealPhase === 'revealing' || revealPhase === 'decrypting'}>{#if revealPhase === 'revealing'}<Loader/> {$t('show.revealing')}{:else if revealPhase === 'decrypting'}<Loader/> {$t('show.decrypting')}{:else}{$t('show.reveal')}{/if}</Button></form><p class="help">{$t('show.fragment_help')}</p></section>{/if}
<style>.center { display: flex; min-height: 18rem; align-items: center; justify-content: center; gap: var(--space-2); }.empty, .gate { max-width: 42rem; margin-inline: auto; padding: var(--space-5); }.empty { display: grid; gap: var(--space-4); text-align: center; }.sleeping { color: var(--accent-600); font: 700 var(--text-xl)/1 var(--font-mono); }.gate header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); margin-bottom: var(--space-4); }.cat { color: var(--accent-600); font: 700 var(--text-xl)/1 var(--font-mono); }dl { display: grid; grid-template-columns: 1fr 1fr; margin-block: var(--space-4); border-block: 1px solid var(--border); }dl div { padding: var(--space-3); }dt { color: var(--ink-muted); font-size: var(--text-sm); }dd { margin: 0; font-weight: 700; }form { display: grid; gap: var(--space-4); }form :global(button) { width: 100%; }.gate > .help { margin-top: var(--space-4); }@media (max-width: 30rem) { .empty, .gate { padding: var(--space-4); } .cat { display: none; } dl { grid-template-columns: 1fr; } }</style>
