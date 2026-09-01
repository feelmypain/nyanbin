<script lang="ts">
	import { apiInfo, errorCodes, operations, type SchemaDoc } from '$lib/api-docs'
</script>

<svelte:head><title>API reference — Nyanbin</title><meta name="description" content="Human-readable reference for the Nyanbin zero-knowledge note API." /></svelte:head>

{#snippet inlineCode(text: string)}
	{#each text.split('`') as part, index}{#if index % 2 === 1}<code>{part}</code>{:else}{part}{/if}{/each}
{/snippet}

{#snippet schemaTable(doc: SchemaDoc)}
	{#if doc.description}<p class="schema-note">{@render inlineCode(doc.description)}</p>{/if}
	{#if doc.rows.length > 0}
		<div class="table-wrap">
			<table>
				<thead><tr><th>Field</th><th>Type</th><th>Required</th><th>Description</th></tr></thead>
				<tbody>
					{#each doc.rows as row}
						<tr><td class="mono" style:padding-inline-start={row.depth ? `calc(var(--space-4) + ${row.depth} * 1.25rem)` : undefined}>{row.name}</td><td class="mono">{row.type}</td><td>{row.required ? 'yes' : 'no'}</td><td>{@render inlineCode(row.description)}</td></tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
{/snippet}

<article class="panel">
	<span class="eyebrow">{apiInfo.title} · protocol {apiInfo.version}</span>
	<h1>API reference</h1>
	<p class="lead">{apiInfo.summary}</p>
	<section>
		<h2>Overview</h2>
		<p>
			Nyanbin is zero-knowledge: encryption and decryption happen entirely in the client, and the
			server only ever stores an opaque encrypted envelope. It cannot read note contents, passwords,
			delete tokens, or link secrets. Creating a note is a two-step flow — <strong>reserve</strong>,
			then encrypt locally, then <strong>commit</strong> — because the server-generated note ID is
			authenticated inside the envelope. There is no idempotency-key mechanism: a lost reserve
			response expires harmlessly and a retry simply reserves anew, while a repeated commit to the
			same ID fails with <code>reservation_mismatch</code>.
		</p>
		<p class="notice warning">
			<strong>Reveal consumes a read.</strong> <code>POST /api/notes/&#123;id&#125;/reveal</code>
			atomically burns one read; on a read-capped note the last reveal deletes it. Use the
			non-consuming <code>GET /api/notes/&#123;id&#125;</code> to inspect lifecycle metadata.
		</p>
		<p>
			The machine-readable specification is served at <a href="/api/openapi.json">/api/openapi.json</a>.
			A reference implementation of the client protocol ships in each versioned
			<a href="https://github.com/feelmypain/nyanbin/releases" rel="noreferrer">GitHub release</a> package.
		</p>
	</section>

	{#each operations as operation (operation.anchor)}
		<section id={operation.anchor}>
			<h2><span class="method">{operation.method}</span> <code class="path">{operation.path}</code></h2>
			<p class="summary">{operation.summary}</p>
			{#if operation.description}<p>{@render inlineCode(operation.description)}</p>{/if}
			{#if operation.params.length > 0}
				<h3>Path parameters</h3>
				<div class="table-wrap">
					<table>
						<thead><tr><th>Name</th><th>Type</th><th>Description</th></tr></thead>
						<tbody>
							{#each operation.params as param (param.name)}
								<tr><td class="mono">{param.name}</td><td class="mono">{param.type}</td><td>{@render inlineCode(param.description)}</td></tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
			{#if operation.request}
				<h3>Request body{#if operation.request.label} — <code>{operation.request.label}</code>{/if}</h3>
				{@render schemaTable(operation.request)}
			{/if}
			<h3>Responses</h3>
			{#each operation.responses as response (response.status)}
				<div class="response">
					<p><strong class="status">{response.status}</strong> {@render inlineCode(response.description)}{#if response.retryAfter}&nbsp;Carries
							a <code>Retry-After</code> header (integer seconds).{/if}</p>
					{#if response.schema && response.schema.rows.length > 0 && response.schema.label !== 'Error'}
						<details>
							<summary>Body — <code>{response.schema.label}</code></summary>
							{@render schemaTable(response.schema)}
						</details>
					{:else if response.schema?.label === 'Error'}
						<p class="schema-note">Body is the uniform <a href="#errors">error object</a>.</p>
					{/if}
				</div>
			{/each}
		</section>
	{/each}

	<section id="errors">
		<h2>Error codes</h2>
		<p>
			Every error response body is exactly <code>&#123; "code", "message" &#125;</code>. Codes are
			machine-readable and append-only; messages are human-readable and may change.
		</p>
		<div class="table-wrap">
			<table>
				<thead><tr><th>Code</th><th>HTTP status</th><th>Meaning</th></tr></thead>
				<tbody>
					{#each errorCodes as row (row.code)}
						<tr><td class="mono">{row.code}</td><td>{row.status}</td><td>{row.meaning}</td></tr>
					{/each}
				</tbody>
			</table>
		</div>
	</section>
</article>

<style>
	article { max-width: 56rem; margin-inline: auto; padding: var(--space-6); }
	h1 { margin-block: var(--space-2) var(--space-3); font-size: var(--text-2xl); }
	.lead { color: var(--ink-soft); font-size: var(--text-lg); }
	section { margin-top: var(--space-6); padding-top: var(--space-5); border-top: 1px solid var(--border); }
	section h2 { margin-bottom: var(--space-2); font-size: var(--text-xl); }
	section h3 { margin-block: var(--space-4) var(--space-2); font-size: var(--text-md, 1rem); }
	section p + p { margin-top: var(--space-3); }
	.method { display: inline-block; padding: .1rem var(--space-3); border: 1px solid var(--border-strong); border-radius: var(--radius-sm); background: var(--surface-accent); color: var(--accent-700); font-family: var(--font-mono); font-size: var(--text-sm); vertical-align: middle; }
	.path { font-size: var(--text-lg); word-break: break-all; }
	.summary { margin-top: var(--space-2); font-weight: 700; }
	.table-wrap { overflow-x: auto; margin-top: var(--space-2); border: 1px solid var(--border); border-radius: var(--radius-sm); }
	table { width: 100%; border-collapse: collapse; font-size: var(--text-sm); }
	th, td { padding: var(--space-2) var(--space-4); border-bottom: 1px solid var(--border); text-align: start; vertical-align: top; }
	thead th { background: var(--surface-accent); }
	tbody tr:last-child td { border-bottom: 0; }
	.mono { font-family: var(--font-mono); white-space: nowrap; }
	.status { font-family: var(--font-mono); color: var(--accent-700); }
	.response { margin-top: var(--space-3); }
	.response details { margin-top: var(--space-2); padding: var(--space-2) var(--space-4); border: 1px solid var(--border); border-radius: var(--radius-sm); }
	.schema-note { margin-top: var(--space-2); color: var(--ink-muted); font-size: var(--text-sm); }
	.notice { margin-top: var(--space-4); }
	@media (max-width: 35rem) { article { padding: var(--space-4); } }
</style>
