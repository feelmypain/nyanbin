<script lang="ts">
	import { t } from 'svelte-intl-precompile'
	import { status } from '$lib/stores/status'
	import ThemeToggle from '$lib/ui/ThemeToggle.svelte'
</script>
<footer><p>{$t('footer.local_only')}</p><nav aria-label={$t('footer.label')}><a href="/about">{$t('nav.about')}</a>{#if $status.state === 'ready' && $status.value.branding.imprintUrl}<a href="/imprint">{$t('imprint.title')}</a>{/if}<a class="source" href="https://github.com/feelmypain/nyanbin" target="_blank" rel="noopener noreferrer">{$t('footer.source')}{#if $status.state === 'ready'}<span class="version" data-testid="instance-version">v{$status.value.version}</span>{/if}</a><ThemeToggle /></nav></footer>
<style>
	footer { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); margin-top: var(--space-7); padding-block: var(--space-5); border-top: 1px solid var(--border); color: var(--ink-muted); }
	nav { display: flex; align-items: center; gap: var(--space-4); }
	.source { display: inline-flex; align-items: center; gap: var(--space-2); }
	.version { padding: .1rem var(--space-2); border: 1px solid var(--border-strong); border-radius: 99rem; background: var(--surface-accent); color: var(--ink-soft); font: 700 var(--text-sm)/1.4 var(--font-mono); }
	@media (max-width: 36rem) { footer { align-items: flex-start; flex-direction: column; } nav { width: 100%; flex-wrap: wrap; justify-content: space-between; } }
</style>
