<script lang="ts">
	import prettyBytes from 'pretty-bytes'
	import { t } from 'svelte-intl-precompile'
	import { safeFilename } from '$lib/utils'
	interface Props { files?: File[] }
	let { files = $bindable([]) }: Props = $props()
	const previewable = new Set(['image/gif', 'image/jpeg', 'image/png', 'image/webp'])
	let urls = $state<string[]>([])
	$effect(() => { const next = files.map((file) => previewable.has(file.type) ? URL.createObjectURL(file) : ''); urls = next; return () => next.forEach((url) => url && URL.revokeObjectURL(url)) })
	function remove(index: number) { files = files.toSpliced(index, 1) }
</script>
{#if files.length}<section aria-labelledby="attachments-title"><div class="heading"><h3 id="attachments-title">{$t('files.selected')}</h3><span>{files.length}</span></div><ul data-testid="attachment-list">{#each files as file, index}<li>{#if urls[index]}<img src={urls[index]} alt=""/>{:else}<span class="file-art" aria-hidden="true">▤</span>{/if}<span class="meta"><strong><bdi>{safeFilename(file.name, $t('files.unnamed'))}</bdi></strong><small>{prettyBytes(file.size)} · {file.type || $t('files.unknown_type')}</small></span><button type="button" data-testid={`remove-file-${index}`} onclick={() => remove(index)} aria-label={$t('files.remove_named', { values: { name: safeFilename(file.name, $t('files.unnamed')) } })}>×</button></li>{/each}</ul></section>{/if}
<style>section { margin-top: var(--space-4); } .heading { display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-2); } .heading span { min-width: 1.5rem; padding-inline: var(--space-1); border-radius: 99rem; background: var(--surface-strong); text-align: center; } ul { display: grid; gap: var(--space-2); margin: 0; padding: 0; list-style: none; } li { display: grid; grid-template-columns: 3rem minmax(0, 1fr) auto; align-items: center; gap: var(--space-3); padding: var(--space-2); border: 1px solid var(--border); border-radius: var(--radius-sm); } img, .file-art { width: 3rem; height: 3rem; object-fit: cover; border-radius: var(--radius-sm); background: var(--surface-blue); } .file-art { display: grid; place-items: center; color: var(--blue-700); font-size: var(--text-xl); } .meta { min-width: 0; } .meta strong, .meta small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .meta small { color: var(--ink-muted); } button { width: 2.75rem; padding: 0; font-size: var(--text-xl); }</style>
