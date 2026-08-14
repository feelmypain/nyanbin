<script lang="ts">
	import { API, NyanbinError, PROTOCOL_VERSION, buildNoteUrl, decodeBase64Url, encodeBase64Url, encryptPayload, generateSecret, hashDeleteToken, type PrivatePayload } from 'nyanbin/shared'
	import { t } from 'svelte-intl-precompile'
	import { status, init as reloadStatus } from '$lib/stores/status'
	import Button from '$lib/ui/Button.svelte'
	import FileUpload from '$lib/ui/FileUpload.svelte'
	import Loader from '$lib/ui/Loader.svelte'
	import MarkdownView from '$lib/ui/MarkdownView.svelte'
	import NoteResult, { type NoteResult as CreatedResult } from '$lib/ui/NoteResult.svelte'
	import PastedFilesPreview from '$lib/ui/PastedFilesPreview.svelte'
	import SourceView from '$lib/ui/SourceView.svelte'
	import Switch from '$lib/ui/Switch.svelte'
	import TextArea from '$lib/ui/TextArea.svelte'
	import TextInput from '$lib/ui/TextInput.svelte'
	import { encryptedEnvelopeBytes, envelopeFileBytes, safeFilename, serializedTextBytes } from '$lib/utils'

	type Format = PrivatePayload['format']
	const fallbackExpiry = 86_400
	let text = $state('')
	let format = $state<Format>('plain')
	let files = $state<File[]>([])
	let preview = $state(false)
	let expiresIn = $state(fallbackExpiry)
	let hasReadCap = $state(true)
	let maxReads = $state(1)
	let hasPassword = $state(false)
	let password = $state('')
	let phase = $state<'idle' | 'reserving' | 'encrypting' | 'uploading'>('idle')
	let error = $state('')
	let result = $state<CreatedResult | null>(null)
	let initialized = $state(false)

	let busy = $derived(phase !== 'idle')
	let empty = $derived(text.length === 0 && files.length === 0)
	let maxEnvelopeBytes = $derived($status.state === 'ready' ? $status.value.limits.maxEnvelopeBytes : 0)
	let envelopeBytes = $derived(encryptedEnvelopeBytes(text, format, files))
	let overageBytes = $derived(Math.max(0, envelopeBytes - maxEnvelopeBytes))
	let tooLarge = $derived(maxEnvelopeBytes > 0 && overageBytes > 0)
	let largestFile = $derived(files.reduce<File | null>((largest, file) => !largest || envelopeFileBytes(file) > envelopeFileBytes(largest) ? file : largest, null))
	let fileIsLargest = $derived(largestFile !== null && envelopeFileBytes(largestFile) >= serializedTextBytes(text))
	let formats = $derived<Format[]>($status.state === 'ready' ? $status.value.capabilities.formats : ['plain'])
	let filesEnabled = $derived($status.state === 'ready' && $status.value.capabilities.files)
	let passwordsEnabled = $derived($status.state === 'ready' && $status.value.capabilities.passwords)
	let expiryChoices = $derived.by(() => {
		const candidates = [3_600, 21_600, 86_400, 604_800, expiresIn]
		const limit = $status.state === 'ready' ? $status.value.limits.maxExpiresIn : fallbackExpiry
		return [...new Set(candidates.filter((value) => value <= limit))].sort((a, b) => a - b)
	})

	$effect(() => {
		if ($status.state !== 'ready') return
		if (!initialized) {
			expiresIn = $status.value.defaults.expiresIn
			hasReadCap = $status.value.defaults.maxReads !== undefined
			maxReads = $status.value.defaults.maxReads ?? 1
			initialized = true
		}
		if (!$status.value.capabilities.formats.includes(format)) format = $status.value.capabilities.formats[0] ?? 'plain'
		if (!$status.value.capabilities.files) files = []
		if (!$status.value.capabilities.passwords) { hasPassword = false; password = '' }
	})

	function addClipboardFiles(event: ClipboardEvent) {
		if (!filesEnabled || busy) return
		const clipboardFiles = Array.from(event.clipboardData?.files ?? [])
		if (clipboardFiles.length === 0) return
		event.preventDefault()
		files = [...files, ...clipboardFiles]
	}

	function createAnother() {
		text = ''
		format = $status.state === 'ready' ? ($status.value.capabilities.formats[0] ?? 'plain') : 'plain'
		files = []
		preview = false
		expiresIn = $status.state === 'ready' ? $status.value.defaults.expiresIn : fallbackExpiry
		hasReadCap = $status.state === 'ready' && $status.value.defaults.maxReads !== undefined
		maxReads = $status.state === 'ready' ? ($status.value.defaults.maxReads ?? 1) : 1
		hasPassword = false
		password = ''
		error = ''
		result = null
		requestAnimationFrame(() => document.querySelector<HTMLElement>('#note-text')?.focus())
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault()
		error = ''
		if (empty) { error = $t('create.errors.empty'); return }
		if ($status.state !== 'ready') { error = $t('create.errors.status'); return }
		if (expiresIn > $status.value.limits.maxExpiresIn || expiresIn < 1) { error = $t('create.errors.expiry'); return }
		if (hasReadCap && (maxReads < 1 || maxReads > $status.value.limits.maxReads)) { error = $t('create.errors.reads'); return }
		if (hasPassword && password.length === 0) { error = $t('create.errors.password'); return }
		if (tooLarge) {
			error = $t('create.errors.too_large_detail', { values: { overage: overageBytes.toLocaleString(), name: fileIsLargest && largestFile ? safeFilename(largestFile.name, $t('files.unnamed')) : $t('create.note_content') } })
			return
		}
		const attachmentNames = new Set<string>()
		for (const file of files) {
			if (file.name.length === 0 || file.name.length > 255 || file.name === '.' || file.name === '..' || /[\\/\0]/.test(file.name)) {
				error = $t('create.errors.file_name')
				return
			}
			if (attachmentNames.has(file.name)) {
				error = $t('create.errors.duplicate_files', { values: { name: safeFilename(file.name, $t('files.unnamed')) } })
				return
			}
			attachmentNames.add(file.name)
		}
		try {
			phase = 'reserving'
			const reservation = await API.reserve({ expiresIn, maxReads: hasReadCap ? maxReads : 0 })
			phase = 'encrypting'
			const privateFiles = await Promise.all(files.map(async (file) => ({ name: file.name, type: file.type || 'application/octet-stream', size: file.size, data: encodeBase64Url(new Uint8Array(await file.arrayBuffer())) })))
			const payload: PrivatePayload = { kind: 'text', format, text, files: privateFiles }
			const secret = generateSecret()
			const envelope = await encryptPayload(payload, { id: reservation.id, lifecycle: reservation.lifecycle, secret, ...(hasPassword ? { password } : {}) })
			if (decodeBase64Url(envelope).byteLength > $status.value.limits.maxEnvelopeBytes) throw new NyanbinError('API_ERROR', 'payload_too_large')
			phase = 'uploading'
			await API.commit(reservation.id, { protocol: PROTOCOL_VERSION, envelope, lifecycle: reservation.lifecycle, deleteTokenHash: await hashDeleteToken(reservation.deleteToken) })
			result = { id: reservation.id, url: buildNoteUrl(window.location.origin, reservation.id, secret), deleteToken: reservation.deleteToken, lifecycle: reservation.lifecycle }
			requestAnimationFrame(() => document.querySelector<HTMLElement>('#result-title')?.focus())
		} catch (cause) {
			if (cause instanceof NyanbinError && (cause.message.includes('payload_too_large') || cause.message.includes('too large'))) error = $t('create.errors.too_large')
			else if (cause instanceof NyanbinError && cause.code === 'NETWORK_ERROR') error = $t('create.errors.network')
			else error = $t('create.errors.failed')
		} finally {
			phase = 'idle'
		}
	}
</script>
{#if result}<NoteResult {result} oncreateanother={createAnother}/>{:else}
<section class="intro"><div><span class="eyebrow">{$t('create.eyebrow')}</span><h1>{$t('create.title')}</h1><p>{$t('create.intro')}</p></div><div class="cat-art" aria-hidden="true"><span class="tail"></span><span class="cat-head">⌁<b>•ᴗ•</b>⌁</span></div></section>
{#if $status.state === 'error'}<div class="notice error status-error" role="alert"><p>{$t('create.errors.status')}</p><Button type="button" onclick={() => reloadStatus()}>{$t('common.retry')}</Button></div>{/if}
{#if $status.state === 'loading'}<p class="notice" role="status" aria-live="polite"><Loader/> {$t('create.loading_instance')}</p>{/if}
<form class="panel composer" data-testid="create-form" onsubmit={submit} onpaste={addClipboardFiles}>
	<fieldset disabled={busy || $status.state !== 'ready'}>
		<div class="editor">
			<div class="format-row"><fieldset class="formats"><legend>{$t('create.format')}</legend>{#each formats as option}<label><input data-testid={`format-${option}`} type="radio" bind:group={format} value={option}/> {$t(`formats.${option}`)}</label>{/each}</fieldset><button class="preview-toggle" type="button" aria-pressed={preview} onclick={() => (preview = !preview)}>{preview ? $t('create.edit') : $t('create.preview')}</button></div>
			{#if preview}<div class="preview" data-testid="content-preview" aria-label={$t('create.preview')}>{#if text}{#if format === 'markdown'}<MarkdownView {text}/>{:else if format === 'source'}<SourceView {text}/>{:else}<pre>{text}</pre>{/if}{:else}<p class="empty-preview">{$t('create.preview_empty')}</p>{/if}</div>{:else}<TextArea id="note-text" data-testid="text-field" label={$t('create.text_label')} help={$t('create.text_help')} bind:value={text} placeholder={$t('create.placeholder')}/>{/if}
			{#if filesEnabled}<FileUpload bind:files disabled={busy || $status.state !== 'ready'}/><PastedFilesPreview bind:files/>{/if}
			<p class="local-note">🔒 <strong>{$t('create.local_title')}</strong> {$t('create.local_body')}</p>
		</div>
		<aside class="options">
			<div><label for="field-expiry"><span>{$t('create.expiry')}</span></label><select id="field-expiry" data-testid="field-expiry" bind:value={expiresIn}>{#each expiryChoices as seconds}<option value={seconds}>{seconds % 86400 === 0 ? $t('expiry.days', { values: { count: seconds / 86400 } }) : seconds % 3600 === 0 ? $t('expiry.hours', { values: { count: seconds / 3600 } }) : $t('expiry.minutes', { values: { count: Math.ceil(seconds / 60) } })}</option>{/each}</select><p class="help">{$t('create.expiry_help')}</p></div>
			<Switch id="read-cap" data-testid="read-cap-toggle" label={$t('create.read_cap')} help={$t('create.read_cap_help')} bind:value={hasReadCap}/>
			{#if hasReadCap}<TextInput id="field-reads" data-testid="field-reads" type="number" min="1" max={$status.state === 'ready' ? $status.value.limits.maxReads : 1} label={$t('create.max_reads')} bind:value={maxReads}/>{/if}
			{#if passwordsEnabled}<Switch id="password-toggle" data-testid="password-toggle" label={$t('create.password_toggle')} help={$t('create.password_help')} bind:value={hasPassword}/>{/if}
			{#if passwordsEnabled && hasPassword}<TextInput id="password" data-testid="password" type="password" reveal autocomplete="new-password" label={$t('common.password')} bind:value={password}/>{/if}
			<details><summary>{$t('create.how_title')}</summary><p>{$t('create.how_body')}</p></details>
			<div class="summary"><span>{files.length} {$t('create.files_count')}</span><span>{$t('create.envelope_size', { values: { bytes: envelopeBytes.toLocaleString() } })}</span>{#if maxEnvelopeBytes}<span>{$t('create.limit', { values: { bytes: maxEnvelopeBytes.toLocaleString() } })}</span>{/if}{#if tooLarge}<strong class="size-error" role="alert">{$t('create.over_limit', { values: { overage: overageBytes.toLocaleString(), name: fileIsLargest && largestFile ? safeFilename(largestFile.name, $t('files.unnamed')) : $t('create.note_content') } })}</strong>{/if}</div>
			<div class="create"><Button variant="primary" style="width:100%" data-testid="create-button" type="submit" disabled={empty || busy || tooLarge || $status.state !== 'ready' || (hasPassword && password.length === 0)}>{#if phase !== 'idle'}<Loader/> {$t(`create.phase.${phase}`)}{:else}{$t('create.submit')}{/if}</Button>{#if empty}<p class="help">{$t('create.add_something')}</p>{:else if hasPassword && password.length === 0}<p class="help">{$t('create.password_required')}</p>{/if}</div>
		</aside>
	</fieldset>
	<div class="live" aria-live="polite" aria-atomic="true">{#if phase !== 'idle'}{$t(`create.phase.${phase}`)}{/if}</div>
	{#if error}<p class="notice error form-error" role="alert">{error}</p>{/if}
</form>
{/if}
<style>
	.intro { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: end; gap: var(--space-5); margin-bottom: var(--space-5); } .intro h1 { max-width: 18ch; font-size: var(--text-2xl); } .intro p { max-width: 60ch; margin-top: var(--space-2); color: var(--ink-soft); font-size: var(--text-lg); } .eyebrow { color: var(--blue-700); font-weight: 700; } .cat-art { position: relative; width: 9rem; height: 4.5rem; color: var(--blue-600); } .cat-head { position: absolute; inset-block-end: 0; inset-inline-end: 0; padding: var(--space-3); border: 2px solid var(--blue-600); border-radius: 45% 45% 35% 35%; background: var(--surface-blue); font: var(--text-sm)/1 var(--font-mono); animation: bob 8s ease-in-out infinite; } .cat-head b { margin-inline: var(--space-1); } .tail { position: absolute; width: 5rem; height: 3rem; inset-block-end: .25rem; inset-inline-start: .5rem; border-block-end: .4rem solid var(--blue-600); border-inline-start: .4rem solid var(--blue-600); border-radius: 0 0 0 100%; transform-origin: 100% 100%; animation: tail 6.5s ease-in-out infinite; } @keyframes bob { 50% { transform: translateY(-.2rem); } } @keyframes tail { 50% { transform: rotate(3deg); } }
	.status-error { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); margin-bottom: var(--space-4); }.composer > fieldset { display: grid; grid-template-columns: minmax(0, 2fr) 18rem; } .editor { min-width: 0; padding: var(--space-5); } .options { display: flex; min-width: 0; flex-direction: column; gap: var(--space-4); padding: var(--space-5); border-inline-start: 1px solid var(--border); background: var(--surface-blue); border-radius: 0 var(--radius-lg) var(--radius-lg) 0; } .format-row { display: flex; align-items: end; justify-content: space-between; gap: var(--space-3); margin-bottom: var(--space-3); } .formats { display: flex; flex-wrap: wrap; gap: var(--space-2) var(--space-4); } .formats legend { width: 100%; } .formats label { display: flex; align-items: center; gap: var(--space-1); cursor: pointer; } .formats input { width: 1.15rem; min-height: 1.15rem; accent-color: var(--blue-600); } .preview-toggle { min-width: 6rem; } .preview { min-height: 18rem; margin-bottom: var(--space-4); padding: var(--space-4); border: 1px solid var(--border-strong); border-radius: var(--radius-sm); overflow: auto; } .preview pre { white-space: pre-wrap; overflow-wrap: anywhere; } .empty-preview { color: var(--ink-muted); } .local-note { margin-block: var(--space-4) 0; color: var(--ink-soft); } details summary { min-height: 2.75rem; cursor: pointer; font-weight: 700; } details p { color: var(--ink-muted); font-size: var(--text-sm); } .summary { display: grid; gap: var(--space-1); padding-block: var(--space-3); border-block: 1px solid var(--border); color: var(--ink-muted); font-size: var(--text-sm); } .create { width: 100%; } .live { min-height: 1px; } .form-error { margin: var(--space-4); }
	:global([dir='rtl']) .options { border-radius: var(--radius-lg) 0 0 var(--radius-lg); }
	:global([dir='rtl']) .tail { border-radius: 0 0 100% 0; transform-origin: 0 100%; animation-name: tail-rtl; }
	@keyframes tail-rtl { 50% { transform: rotate(-3deg); } }
	@media (max-width: 64rem) { .composer > fieldset { grid-template-columns: 1fr; } .options { display: grid; grid-template-columns: 1fr 1fr; border-inline-start: 0; border-block-start: 1px solid var(--border); border-radius: 0 0 var(--radius-lg) var(--radius-lg); } :global([dir='rtl']) .options { border-radius: 0 0 var(--radius-lg) var(--radius-lg); } .options .create, .options details, .options .summary { grid-column: 1 / -1; } }
	@media (max-width: 48rem) { .cat-art { width: 5rem; transform: scale(.8); transform-origin: bottom right; } :global([dir='rtl']) .cat-art { transform-origin: bottom left; } .editor, .options { padding: var(--space-4); } .preview { min-height: 13.75rem; } }
	@media (max-width: 36rem) { .intro { grid-template-columns: 1fr; } .cat-art { display: none; } .format-row { align-items: stretch; flex-direction: column; } .formats { gap: var(--space-2); } .formats label { width: 100%; min-height: 2.75rem; } .preview-toggle { width: 100%; } .options { grid-template-columns: 1fr; } .status-error { align-items: stretch; flex-direction: column; } }
</style>
