<script lang="ts" module>
	type Part = { text: string; kind: 'plain' | 'keyword' | 'string' | 'number' | 'comment' }
	const keywords = new Set(['async','await','break','case','catch','class','const','continue','def','default','delete','do','else','export','extends','false','finally','for','from','function','if','import','in','interface','let','new','null','of','return','static','switch','throw','true','try','type','undefined','var','while','yield'])
	function colorize(line: string): Part[] {
		const out: Part[] = []
		const matcher = /(\/\/.*|#.*|\/\*[\s\S]*?\*\/|'(?:\\.|[^'\\])*'|"(?:\\.|[^"\\])*"|`(?:\\.|[^`\\])*`|\b\d+(?:\.\d+)?\b|\b[A-Za-z_$][\w$]*\b)/g
		let cursor = 0
		for (const match of line.matchAll(matcher)) {
			if ((match.index ?? 0) > cursor) out.push({ text: line.slice(cursor, match.index), kind: 'plain' })
			const text = match[0]
			let kind: Part['kind'] = 'plain'
			if (text.startsWith('//') || text.startsWith('#') || text.startsWith('/*')) kind = 'comment'
			else if (/^['"`]/.test(text)) kind = 'string'
			else if (/^\d/.test(text)) kind = 'number'
			else if (keywords.has(text)) kind = 'keyword'
			out.push({ text, kind }); cursor = (match.index ?? 0) + text.length
		}
		if (cursor < line.length) out.push({ text: line.slice(cursor), kind: 'plain' })
		return out
	}
</script>
<script lang="ts">interface Props { text: string }; let { text }: Props = $props(); let lines = $derived(text.split('\n'))</script>
<pre><code>{#each lines as line, index}<span class="line"><span class="number" aria-hidden="true">{index + 1}</span><span>{#each colorize(line) as part}<span class={part.kind}>{part.text}</span>{/each}</span></span>{/each}</code></pre>
<style>pre { margin: 0; padding: var(--space-4); overflow: auto; border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--surface-accent); font-size: var(--text-sm); line-height: 1.6; tab-size: 2; } .line { display: grid; grid-template-columns: 3ch minmax(0, 1fr); min-height: 1.6em; } .number { color: var(--ink-muted); user-select: none; } .keyword { color: var(--accent-700); font-weight: 700; } .string { color: var(--success); } .comment { color: var(--ink-muted); font-style: italic; } .number:not(:first-child) { color: var(--danger); }</style>
