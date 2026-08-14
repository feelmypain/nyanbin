<script lang="ts">
	import { encode } from 'uqr'
	import { t } from 'svelte-intl-precompile'
	interface Props { value: string }
	let { value }: Props = $props()
	let code = $derived(encode(value, { ecc: 'Q' }))
	const border = 2
</script>
<figure data-testid="share-qr"><svg viewBox={`0 0 ${code.size + border * 2} ${code.size + border * 2}`} role="img" aria-label={$t('result.qr_label')} shape-rendering="crispEdges"><rect width="100%" height="100%" class="paper"/>{#each code.data as row, y}{#each row as dark, x}{#if dark}<rect x={x + border} y={y + border} width="1" height="1" class="ink"/>{/if}{/each}{/each}</svg><figcaption>{$t('result.qr_caption')}</figcaption></figure>
<style>figure { width: min(14rem, 100%); margin: 0; } svg { display: block; width: 100%; height: auto; padding: var(--space-2); border: 1px solid var(--border-strong); border-radius: var(--radius-sm); background: var(--qr-paper); } .paper { fill: var(--qr-paper); } .ink { fill: var(--qr-ink); } figcaption { margin-top: var(--space-1); color: var(--ink-muted); font-size: var(--text-sm); }</style>
