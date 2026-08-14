<script lang="ts">
	import { t } from 'svelte-intl-precompile'
	interface Props { files?: File[]; disabled?: boolean; onfiles?: (files: File[]) => void }
	let { files = $bindable([]), disabled = false, onfiles }: Props = $props()
	let dragging = $state(false)
	function add(list: FileList | null) { if (disabled || !list) return; const added = Array.from(list); files = [...files, ...added]; onfiles?.(added) }
	function input(event: Event) { const picker = event.currentTarget as HTMLInputElement; add(picker.files); picker.value = '' }
	function drop(event: DragEvent) { event.preventDefault(); dragging = false; if (!disabled) add(event.dataTransfer?.files ?? null) }
</script>
<div role="group" aria-label={$t('files.add')} aria-disabled={disabled} class:dragging class="drop" ondragover={(event) => { event.preventDefault(); if (!disabled) dragging = true }} ondragleave={() => (dragging = false)} ondrop={drop}>
	<label><span>{$t('files.add')}</span><input data-testid="file-upload" type="file" multiple {disabled} onchange={input}/><small>{$t('files.drop_help')}</small></label>
</div>
<style>.drop { display: grid; min-height: 7rem; place-items: center; padding: var(--space-4); border: 2px dashed var(--border-strong); border-radius: var(--radius-md); background: var(--surface-accent); text-align: center; transition: background var(--duration-ui) var(--ease-out), border-color var(--duration-ui) var(--ease-out); } .drop:hover:not([aria-disabled='true']), .drop.dragging { border-color: var(--accent-600); background: var(--surface-strong); } label { width: 100%; cursor: pointer; } input { margin-block: var(--space-2); padding: var(--space-2); background: var(--surface); } small { display: block; color: var(--ink-muted); }</style>
