<script lang="ts">
	import { onMount } from 'svelte'
	import { get } from 'svelte/store'
	import { locale, t } from 'svelte-intl-precompile'
	import '../app.css'
	import { init } from '$lib/stores/status'
	import Footer from '$lib/views/Footer.svelte'
	import Header from '$lib/views/Header.svelte'
	interface Props { children?: import('svelte').Snippet }
	let { children }: Props = $props()
	onMount(() => {
		void init()
		const update = (value: string) => { document.documentElement.lang = value; document.documentElement.dir = /^(ar|fa|he|ur)(-|$)/i.test(value) ? 'rtl' : 'ltr' }
		update(get(locale))
		return locale.subscribe(update)
	})
</script>
<svelte:head><script src="/theme-init.js"></script><title>Nyanbin — encrypted sharing</title><meta name="description" content="Share encrypted notes and files with a link secret that stays in your browser."/><meta name="referrer" content="no-referrer"/><link rel="icon" href="/favicon.svg"/></svelte:head>
<a class="skip-link" href="#main">{$t('nav.skip')}</a>
<div class="shell"><Header /><main id="main">{@render children?.()}</main><Footer /></div>
<style>.shell { width: min(calc(100% - 2 * var(--space-5)), 70rem); margin-inline: auto; } main { min-height: 60vh; } @media (max-width: 30rem) { .shell { width: min(calc(100% - 2 * var(--space-4)), 70rem); } }</style>
