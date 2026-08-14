<script lang="ts" module>
	import { writable } from 'svelte/store'
	export type Theme = 'system' | 'light' | 'dark'
	function initial(): Theme {
		if (typeof window === 'undefined') return 'system'
		try {
			const value = localStorage.getItem('nyanbin-theme')
			return value === 'light' || value === 'dark' ? value : 'system'
		} catch {
			return 'system'
		}
	}
	export const theme = writable<Theme>(initial())
	theme.subscribe((value) => {
		if (typeof document === 'undefined') return
		try { localStorage.setItem('nyanbin-theme', value) } catch { /* Keep the selected in-memory theme. */ }
		if (value === 'system') document.documentElement.removeAttribute('theme')
		else document.documentElement.setAttribute('theme', value)
	})
</script>
<script lang="ts">
	import { t } from 'svelte-intl-precompile'
	function change(event: Event) { theme.set((event.currentTarget as HTMLSelectElement).value as Theme) }
</script>
<label class="theme"><span class="sr-only">{$t('theme.label')}</span><select value={$theme} onchange={change} aria-label={$t('theme.label')}><option value="system">{$t('theme.system')}</option><option value="light">{$t('theme.light')}</option><option value="dark">{$t('theme.dark')}</option></select></label>
<style>.theme select { min-height: 2.75rem; padding-block: var(--space-2); }</style>
