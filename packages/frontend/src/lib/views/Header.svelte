<script lang="ts">
	import { goto } from '$app/navigation'
	import { availableLocales } from '$locales'
	import { locale, t } from 'svelte-intl-precompile'
	import { leaveGuard } from '$lib/stores/leave-guard'
	let dialog = $state<HTMLDialogElement | null>(null)
	function brandClick(event: MouseEvent) {
		if (!$leaveGuard) return
		event.preventDefault()
		dialog?.showModal()
	}
	function stay() { dialog?.close() }
	function leave() {
		dialog?.close()
		const proceed = $leaveGuard
		leaveGuard.set(null)
		proceed?.()
		goto('/')
	}
	const nativeNames: Record<string, string> = { ar: 'العربية', cs: 'Čeština', de: 'Deutsch', en: 'English', es: 'Español', fr: 'Français', it: 'Italiano', ja: '日本語', pl: 'Polski', ru: 'Русский', zh: '简体中文', 'zh-TW': '繁體中文' }
	function nativeName(value: string): string {
		if (nativeNames[value]) return nativeNames[value]
		try { return new Intl.DisplayNames([value], { type: 'language' }).of(value) ?? value } catch { return value }
	}
	function changeLocale(event: Event) {
		const value = (event.currentTarget as HTMLSelectElement).value
		locale.set(value)
		try { localStorage.setItem('nyanbin-locale', value) } catch { /* The in-memory choice still applies. */ }
	}
</script>
<header>
	<a class="brand" href="/" aria-label={$t('nav.home')} onclick={brandClick}>
		<svg viewBox="0 0 64 56" aria-hidden="true"><path class="head" d="M10 24 15 5l15 12h4L49 5l5 19v19c0 7-7 11-22 11S10 50 10 43Z"/><path class="face" d="M22 34h1m18 0h1M25 43c4 3 10 3 14 0M14 38 3 35m11 8L2 45m48-7 11-3m-11 8 12 2"/></svg>
		<span>Nyanbin</span>
	</a>
	<nav aria-label={$t('nav.label')}><a href="/about">{$t('nav.about')}</a><label><span class="sr-only">{$t('locale.label')}</span><select value={$locale} onchange={changeLocale} aria-label={$t('locale.label')}>{#each availableLocales as option}<option value={option}>{nativeName(option)}</option>{/each}</select></label></nav>
</header>
<dialog bind:this={dialog} aria-labelledby="leave-title">
	<span class="dialog-cat" aria-hidden="true">/ᐠ｡ꞈ｡ᐟ\</span>
	<h2 id="leave-title">{$t('leave.title')}</h2>
	<p>{$t('leave.body')}</p>
	<div class="dialog-actions">
		<button class="primary" type="button" onclick={stay}>{$t('leave.stay')}</button>
		<button type="button" data-testid="leave-confirm" onclick={leave}>{$t('leave.confirm')}</button>
	</div>
</dialog>
<style>
	header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); padding-block: var(--space-5); }
	.brand { display: inline-flex; align-items: center; gap: var(--space-2); color: var(--ink); text-decoration: none; font: 700 var(--text-xl)/1 var(--font-display); }
	.brand:hover svg { transform: rotate(-4deg) translateY(-.1rem); }
	svg { width: 3.25rem; height: 2.9rem; overflow: visible; transition: transform var(--duration-ui) var(--ease-out); }
	.head { fill: var(--surface-strong); stroke: var(--accent-700); stroke-width: 3; stroke-linejoin: round; }
	.face { fill: none; stroke: var(--accent-700); stroke-width: 3; stroke-linecap: round; }
	nav { display: flex; align-items: center; gap: var(--space-4); }
	nav a { display: inline-flex; align-items: center; min-height: 2.75rem; padding-inline: var(--space-3); border-radius: 99rem; color: var(--accent-700); font-weight: 700; text-decoration: none; transition: background var(--duration-ui) var(--ease-out); }
	nav a:hover { background: var(--surface-accent); text-decoration: underline; }
	nav select { min-height: 2.75rem; width: auto; padding: var(--space-2) var(--space-3); padding-inline-end: 2.4rem; }
	dialog { width: min(26rem, calc(100vw - 2 * var(--space-4))); padding: var(--space-5); border: 1px solid var(--border); border-radius: var(--radius-lg); background: var(--surface); color: var(--ink); box-shadow: var(--shadow-panel); }
	dialog::backdrop { background: color-mix(in srgb, var(--ink) 42%, transparent); backdrop-filter: blur(2px); }
	.dialog-cat { display: block; margin-bottom: var(--space-2); color: var(--accent-600); font: 700 var(--text-xl)/1 var(--font-mono); }
	dialog h2 { margin-bottom: var(--space-3); font-size: var(--text-xl); }
	dialog p { margin-bottom: var(--space-4); color: var(--ink-soft); }
	.dialog-actions { display: flex; flex-wrap: wrap; gap: var(--space-2); } .dialog-actions button { flex: 1; }
	@media (max-width: 30rem) { header { align-items: flex-start; } .brand span { font-size: var(--text-lg); } nav { gap: var(--space-2); } nav a { display: none; } }
</style>
