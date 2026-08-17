<script lang="ts" module>
	import type { Lifecycle } from 'nyanbin/shared'
	export type NoteResult = { id: string; url: string; deleteToken: string; lifecycle: Lifecycle; passwordProtected: boolean }
</script>
<script lang="ts">
	import { onDestroy } from 'svelte'
	import { API, buildShortUrl } from 'nyanbin/shared'
	import { t, locale } from 'svelte-intl-precompile'
	import { copy, formatExpiry } from '$lib/utils'
	import { leaveGuard } from '$lib/stores/leave-guard'
	import Button from './Button.svelte'
	import QR from './QR.svelte'
	interface Props { result: NoteResult; oncreateanother: () => void }
	let { result, oncreateanother }: Props = $props()
	let copied = $state(false); let busy = $state(false); let copyError = $state(''); let revokeError = $state(''); let revoked = $state(false)
	let shortUrl = $state(''); let shortBusy = $state(false); let shortError = $state(''); let shortCopied = $state(false)
	async function copyLink() { copied = await copy(result.url); copyError = copied ? '' : $t('result.copy_failed') }
	async function copyShort() { shortCopied = await copy(shortUrl); shortError = shortCopied ? '' : $t('result.copy_failed') }
	async function mintShort() {
		shortBusy = true
		shortError = ''
		try {
			const { code } = await API.createShort(result.id, result.deleteToken)
			shortUrl = buildShortUrl(window.location.origin, code) + new URL(result.url).hash
		} catch {
			shortError = $t('result.short_failed')
		} finally {
			shortBusy = false
		}
	}
	async function revoke() {
		busy = true
		revokeError = ''
		try {
			await API.deleteNote(result.id, result.deleteToken)
			revoked = true
			requestAnimationFrame(() => document.querySelector<HTMLElement>('#result-title')?.focus())
		} catch {
			revokeError = $t('result.revoke_failed')
		} finally {
			busy = false
		}
	}
	$effect(() => {
		leaveGuard.set(revoked ? null : () => oncreateanother())
	})
	onDestroy(() => leaveGuard.set(null))
</script>
<section class="panel result" data-testid="create-result" aria-labelledby="result-title">
	{#if revoked}<div class="notice success" role="status"><h2 id="result-title" tabindex="-1">{$t('result.revoked_title')}</h2><p>{$t('result.revoked_body')}</p></div><div class="new"><Button type="button" onclick={oncreateanother}>{$t('result.create_another')}</Button></div>
	{:else}<header><div><span class="eyebrow">{$t('result.step')}</span><h2 id="result-title" tabindex="-1">{$t('result.title')}</h2></div><span class="cat" aria-hidden="true">ฅ^•ﻌ•^ฅ</span></header><p class="notice warning">{$t('result.bearer_warning')}</p><label><span>{$t('result.share_link')}</span><textarea data-testid="share-link" readonly rows="3" value={result.url}></textarea></label><div class="actions"><Button variant="primary" data-testid="copy-link" type="button" onclick={copyLink}>{copied ? $t('result.copied') : $t('result.copy')}</Button><a class="button-link" href={result.url} target="_blank" rel="noopener noreferrer">{$t('result.open')}</a><Button type="button" data-testid="create-another" onclick={oncreateanother}>{$t('result.create_another')}</Button></div>{#if copyError}<p class="notice error copy-error" role="alert">{copyError}</p>{/if}<span class="sr-only" aria-live="polite">{copied ? $t('result.copied') : ''}</span><div class="details"><QR value={result.url}/><dl><div><dt>{$t('result.expires')}</dt><dd>{formatExpiry(result.lifecycle.expiresAt, $locale)}</dd></div><div><dt>{$t('result.reads')}</dt><dd>{result.lifecycle.maxReads ?? $t('result.unlimited_reads')}</dd></div></dl></div>{#if result.passwordProtected}<section class="short" aria-labelledby="short-title"><h3 id="short-title">{$t('result.short_summary')}</h3><p>{$t('result.short_help')}</p>{#if shortUrl}<label><span>{$t('result.short_link')}</span><textarea data-testid="short-link" readonly rows="2" value={shortUrl}></textarea></label><div class="actions"><Button variant="primary" data-testid="copy-short" type="button" onclick={copyShort}>{shortCopied ? $t('result.copied') : $t('result.copy')}</Button></div><span class="sr-only" aria-live="polite">{shortCopied ? $t('result.copied') : ''}</span>{:else}<Button data-testid="create-short" disabled={shortBusy} type="button" onclick={mintShort}>{shortBusy ? $t('common.working') : $t('result.short_create')}</Button>{/if}{#if shortError}<p class="notice error short-error" role="alert">{shortError}</p>{/if}</section>{/if}<section class="creator" aria-labelledby="creator-title"><h3 id="creator-title">{$t('result.revoke_summary')}</h3><p>{$t('result.revoke_help')}</p><Button variant="danger" data-testid="revoke-button" disabled={busy} type="button" onclick={revoke}>{busy ? $t('common.working') : $t('result.revoke')}</Button>{#if revokeError}<div class="notice error revoke-error" role="alert"><span class="error-cat" aria-hidden="true">(=ｘﻌｘ=)</span><p>{revokeError}</p></div>{/if}</section>{/if}
</section>
<style>
	.result { padding: var(--space-5); } header { display: flex; justify-content: space-between; gap: var(--space-4); align-items: center; margin-bottom: var(--space-4); } header h2 { margin-top: var(--space-3); } .cat { color: var(--accent-600); font: 700 var(--text-xl)/1 var(--font-mono); } label { display: block; margin-top: var(--space-4); } textarea { overflow-wrap: anywhere; font-family: var(--font-mono); } .actions { display: flex; flex-wrap: wrap; gap: var(--space-2); margin-block: var(--space-3) var(--space-5); } .copy-error { margin-block: calc(var(--space-4) * -1) var(--space-5); } .button-link { display: inline-flex; min-height: 2.75rem; align-items: center; padding: var(--space-2) var(--space-4); border: 1px solid var(--border-strong); border-radius: var(--radius-sm); background: var(--surface); font-weight: 700; text-decoration: none; box-shadow: var(--shadow-keyline); transition: border-color var(--duration-ui) var(--ease-out), background var(--duration-ui) var(--ease-out), transform var(--duration-fast) var(--ease-out); } .button-link:hover { border-color: var(--accent-600); background: var(--surface-accent); transform: translateY(-1px); } .details { display: grid; grid-template-columns: minmax(10rem, 14rem) 1fr; gap: var(--space-5); align-items: start; } dl { margin: 0; } dl div { padding: var(--space-3); border-bottom: 1px solid var(--border); } dt { color: var(--ink-muted); font-size: var(--text-sm); } dd { margin: 0; font-weight: 700; } .short { margin-top: var(--space-5); padding-top: var(--space-4); border-top: 1px solid var(--border); } .short h3 { margin-bottom: var(--space-2); font-size: var(--text-md); } .short p { margin-bottom: var(--space-3); color: var(--ink-muted); } .short label { margin-top: var(--space-3); } .short .actions { margin-block: var(--space-3) 0; } .short-error { margin-top: var(--space-3); } .creator { margin-top: var(--space-5); padding-top: var(--space-4); border-top: 1px solid var(--border); } .creator h3 { margin-bottom: var(--space-2); font-size: var(--text-md); } .creator p { margin-bottom: var(--space-3); color: var(--ink-muted); } .revoke-error { display: flex; align-items: center; gap: var(--space-3); margin-top: var(--space-4); padding-block: var(--space-3); animation: revoke-pop var(--duration-ui) var(--ease-out); } .revoke-error .error-cat { flex: 0 0 auto; font: 700 var(--text-md)/1 var(--font-mono); } .revoke-error p { margin: 0; color: inherit; } @keyframes revoke-pop { from { opacity: 0; transform: translateY(-.35rem); } } .new { display: inline-block; margin-top: var(--space-4); } @media (max-width: 35rem) { .result { padding: var(--space-4); } .details { grid-template-columns: 1fr; } .actions > * { width: 100%; justify-content: center; } }
</style>
