<script lang="ts" module>
	import type { Lifecycle } from 'nyanbin/shared'
	export type NoteResult = { id: string; url: string; deleteToken: string; lifecycle: Lifecycle }
</script>
<script lang="ts">
	import { API } from 'nyanbin/shared'
	import { t, locale } from 'svelte-intl-precompile'
	import { copy, formatExpiry } from '$lib/utils'
	import Button from './Button.svelte'
	import QR from './QR.svelte'
	interface Props { result: NoteResult; oncreateanother: () => void }
	let { result, oncreateanother }: Props = $props()
	let copied = $state(false); let busy = $state(false); let error = $state(''); let revoked = $state(false)
	async function copyLink() { copied = await copy(result.url); error = copied ? '' : $t('result.copy_failed') }
	async function revoke() {
		busy = true
		error = ''
		try {
			await API.delete(result.id, result.deleteToken)
			revoked = true
			requestAnimationFrame(() => document.querySelector<HTMLElement>('#result-title')?.focus())
		} catch {
			error = $t('result.revoke_failed')
		} finally {
			busy = false
		}
	}
</script>
<section class="panel result" data-testid="create-result" aria-labelledby="result-title">
	{#if revoked}<div class="notice success" role="status"><h2 id="result-title" tabindex="-1">{$t('result.revoked_title')}</h2><p>{$t('result.revoked_body')}</p></div><div class="new"><Button type="button" onclick={oncreateanother}>{$t('result.create_another')}</Button></div>
	{:else}<header><div><span class="eyebrow">{$t('result.step')}</span><h2 id="result-title" tabindex="-1">{$t('result.title')}</h2></div><span class="cat" aria-hidden="true">ฅ^•ﻌ•^ฅ</span></header><p class="notice warning">{$t('result.bearer_warning')}</p><label><span>{$t('result.share_link')}</span><textarea data-testid="share-link" readonly rows="3" value={result.url}></textarea></label><div class="actions"><Button variant="primary" data-testid="copy-link" type="button" onclick={copyLink}>{copied ? $t('result.copied') : $t('result.copy')}</Button><a class="button-link" href={result.url} target="_blank" rel="noopener noreferrer">{$t('result.open')}</a><Button type="button" data-testid="create-another" onclick={oncreateanother}>{$t('result.create_another')}</Button></div><span class="sr-only" aria-live="polite">{copied ? $t('result.copied') : ''}</span><div class="details"><QR value={result.url}/><dl><div><dt>{$t('result.expires')}</dt><dd>{formatExpiry(result.lifecycle.expiresAt, $locale)}</dd></div><div><dt>{$t('result.reads')}</dt><dd>{result.lifecycle.maxReads ?? $t('result.unlimited_reads')}</dd></div></dl></div><section class="creator" aria-labelledby="creator-title"><h3 id="creator-title">{$t('result.revoke_summary')}</h3><p>{$t('result.revoke_help')}</p><Button variant="danger" data-testid="revoke-button" disabled={busy} type="button" onclick={revoke}>{busy ? $t('common.working') : $t('result.revoke')}</Button></section>{#if error}<p class="notice error" role="alert">{error}</p>{/if}{/if}
</section>
<style>
	.result { padding: var(--space-5); } header { display: flex; justify-content: space-between; gap: var(--space-4); align-items: center; margin-bottom: var(--space-4); } .cat { color: var(--blue-600); font: 700 var(--text-xl)/1 var(--font-mono); } label { display: block; margin-top: var(--space-4); } textarea { overflow-wrap: anywhere; font-family: var(--font-mono); } .actions { display: flex; flex-wrap: wrap; gap: var(--space-2); margin-block: var(--space-3) var(--space-5); } .button-link { display: inline-flex; min-height: 2.75rem; align-items: center; padding: var(--space-2) var(--space-4); border: 1px solid var(--border-strong); border-radius: var(--radius-sm); background: var(--surface); font-weight: 700; text-decoration: none; box-shadow: var(--shadow-keyline); transition: border-color var(--duration-ui) var(--ease-out), background var(--duration-ui) var(--ease-out), transform var(--duration-fast) var(--ease-out); } .button-link:hover { border-color: var(--blue-600); background: var(--surface-blue); transform: translateY(-1px); } .details { display: grid; grid-template-columns: minmax(10rem, 14rem) 1fr; gap: var(--space-5); align-items: start; } dl { margin: 0; } dl div { padding: var(--space-3); border-bottom: 1px solid var(--border); } dt { color: var(--ink-muted); font-size: var(--text-sm); } dd { margin: 0; font-weight: 700; } .creator { margin-top: var(--space-5); padding-top: var(--space-4); border-top: 1px solid var(--border); } .creator h3 { margin-bottom: var(--space-2); font-size: var(--text-md); } .creator p { margin-bottom: var(--space-3); color: var(--ink-muted); } .new { display: inline-block; margin-top: var(--space-4); } @media (max-width: 35rem) { .result { padding: var(--space-4); } .details { grid-template-columns: 1fr; } .actions > * { width: 100%; justify-content: center; } }
</style>
