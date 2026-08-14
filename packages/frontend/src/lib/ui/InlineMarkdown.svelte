<script lang="ts" module>
	type Inline = { kind: 'text' | 'strong' | 'em' | 'code' | 'link'; text: string; href?: string }
	function tokenize(value: string): Inline[] {
		const output: Inline[] = []; const pattern = /(`[^`]+`|\*\*[^*]+\*\*|\*[^*]+\*|\[[^\]]+\]\([^)]+\))/g; let cursor = 0
		for (const match of value.matchAll(pattern)) {
			if ((match.index ?? 0) > cursor) output.push({ kind: 'text', text: value.slice(cursor, match.index) })
			const token = match[0]
			if (token.startsWith('`')) output.push({ kind: 'code', text: token.slice(1, -1) })
			else if (token.startsWith('**')) output.push({ kind: 'strong', text: token.slice(2, -2) })
			else if (token.startsWith('*')) output.push({ kind: 'em', text: token.slice(1, -1) })
			else { const parts = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(token); let href: string | undefined; try { const url = new URL(parts?.[2] ?? ''); if (url.protocol === 'http:' || url.protocol === 'https:' || url.protocol === 'mailto:') href = url.toString() } catch { href = undefined } output.push(href ? { kind: 'link', text: parts?.[1] ?? token, href } : { kind: 'text', text: token }) }
			cursor = (match.index ?? 0) + token.length
		}
		if (cursor < value.length) output.push({ kind: 'text', text: value.slice(cursor) })
		return output
	}
</script>
<script lang="ts">interface Props { text: string }; let { text }: Props = $props(); let parts = $derived(tokenize(text))</script>
{#each parts as part}{#if part.kind === 'strong'}<strong>{part.text}</strong>{:else if part.kind === 'em'}<em>{part.text}</em>{:else if part.kind === 'code'}<code>{part.text}</code>{:else if part.kind === 'link'}<a href={part.href} target="_blank" rel="noopener noreferrer">{part.text}</a>{:else}{part.text}{/if}{/each}
