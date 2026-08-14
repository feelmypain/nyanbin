<script lang="ts" module>
	type Block =
		| { kind: 'h1' | 'h2' | 'h3' | 'p' | 'quote' | 'code' | 'rule'; text: string }
		| { kind: 'list'; items: string[] }
	function parse(text: string): Block[] {
		const lines = text.replace(/\r\n?/g, '\n').split('\n'); const blocks: Block[] = []; let paragraph: string[] = []; let code: string[] | null = null; let listOpen = false
		const flush = () => { if (paragraph.length) { blocks.push({ kind: 'p', text: paragraph.join(' ') }); paragraph = [] } }
		for (const line of lines) {
			if (/^```/.test(line)) { flush(); if (code) { blocks.push({ kind: 'code', text: code.join('\n') }); code = null } else code = []; continue }
			if (code) { code.push(line); continue }
			if (!line.trim()) { flush(); listOpen = false; continue }
			const heading = /^(#{1,3})\s+(.+)$/.exec(line); if (heading) { flush(); listOpen = false; blocks.push({ kind: `h${heading[1].length}` as 'h1' | 'h2' | 'h3', text: heading[2] }); continue }
			const item = /^\s*[-*+]\s+(.+)$/.exec(line); if (item) { flush(); const previous = blocks.at(-1); if (listOpen && previous?.kind === 'list') previous.items.push(item[1]); else blocks.push({ kind: 'list', items: [item[1]] }); listOpen = true; continue }
			const quote = /^>\s?(.+)$/.exec(line); if (quote) { flush(); listOpen = false; blocks.push({ kind: 'quote', text: quote[1] }); continue }
			if (/^\s*([-*_])\1\1+\s*$/.test(line)) { flush(); listOpen = false; blocks.push({ kind: 'rule', text: '' }); continue }
			paragraph.push(line.trim())
		}
		if (code) blocks.push({ kind: 'code', text: code.join('\n') }); flush(); return blocks
	}
</script>
<script lang="ts">
	import InlineMarkdown from './InlineMarkdown.svelte'
	interface Props { text: string }
	let { text }: Props = $props()
	let blocks = $derived(parse(text))
</script>
<div class="markdown">{#each blocks as block}{#if block.kind === 'h1'}<h2><InlineMarkdown text={block.text}/></h2>{:else if block.kind === 'h2'}<h3><InlineMarkdown text={block.text}/></h3>{:else if block.kind === 'h3'}<h4><InlineMarkdown text={block.text}/></h4>{:else if block.kind === 'quote'}<blockquote><InlineMarkdown text={block.text}/></blockquote>{:else if block.kind === 'list'}<ul>{#each block.items as item}<li><InlineMarkdown text={item}/></li>{/each}</ul>{:else if block.kind === 'code'}<pre><code>{block.text}</code></pre>{:else if block.kind === 'rule'}<hr/>{:else}<p><InlineMarkdown text={block.text}/></p>{/if}{/each}</div>
<style>.markdown { overflow-wrap: anywhere; } .markdown :global(*) { max-width: 100%; } h2, h3, h4, p, blockquote, ul, pre { margin: 0 0 var(--space-4); } h2 { font-size: var(--text-xl); } h3 { font-size: var(--text-lg); } h4 { font-size: var(--text-md); } blockquote { padding-inline-start: var(--space-4); border-inline-start: .25rem solid var(--border-strong); color: var(--ink-soft); } ul { padding-inline-start: var(--space-5); } pre { padding: var(--space-4); overflow: auto; border-radius: var(--radius-sm); background: var(--surface-accent); white-space: pre-wrap; } hr { border: 0; border-top: 1px solid var(--border); } </style>
