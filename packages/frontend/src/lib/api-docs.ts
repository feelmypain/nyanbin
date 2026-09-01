/**
 * Build-time transformation of the machine-readable OpenAPI document into the
 * static structures rendered by the /docs/api page. The JSON is imported and
 * bundled at build time; nothing here fetches at runtime.
 */
import specJson from '../../../backend/openapi.json'

type SchemaObject = {
	$ref?: string
	type?: string | string[]
	format?: string
	description?: string
	properties?: Record<string, SchemaObject>
	required?: string[]
	items?: SchemaObject
	enum?: Array<string | number>
	const?: string | number
	minimum?: number
	maximum?: number
	minLength?: number
	maxLength?: number
	pattern?: string
	default?: unknown
	'x-docs-hidden'?: boolean
}

type ParameterObject = {
	$ref?: string
	name?: string
	in?: string
	required?: boolean
	description?: string
	schema?: SchemaObject
}

type MediaObject = { schema?: SchemaObject }

type ResponseObject = {
	$ref?: string
	description?: string
	content?: Record<string, MediaObject>
	headers?: Record<string, unknown>
}

type OperationObject = {
	operationId?: string
	summary?: string
	description?: string
	requestBody?: { description?: string; content?: Record<string, MediaObject> }
	responses?: Record<string, ResponseObject>
	parameters?: ParameterObject[]
}

type PathItemObject = { parameters?: ParameterObject[] } & Partial<
	Record<'get' | 'put' | 'post' | 'delete' | 'patch', OperationObject>
>

type OpenApiDocument = {
	info: { title: string; version: string; summary?: string }
	paths: Record<string, PathItemObject>
	components?: {
		schemas?: Record<string, SchemaObject>
		responses?: Record<string, ResponseObject>
		parameters?: Record<string, ParameterObject>
	}
}

const spec = specJson as unknown as OpenApiDocument

function resolve<T extends { $ref?: string }>(value: T): T {
	const ref = value.$ref
	if (!ref) return value
	const segments = ref.replace(/^#\//, '').split('/')
	let cursor: unknown = spec
	for (const segment of segments) {
		if (typeof cursor !== 'object' || cursor === null) return value
		cursor = (cursor as Record<string, unknown>)[segment]
	}
	return (cursor as T) ?? value
}

function refName(ref: string | undefined): string {
	if (!ref) return ''
	const index = ref.lastIndexOf('/')
	return index === -1 ? ref : ref.slice(index + 1)
}

function singleLine(text: string | undefined): string {
	return (text ?? '').replace(/\s+/g, ' ').trim()
}

function typeLabel(schema: SchemaObject): string {
	if (schema.$ref) return refName(schema.$ref)
	if (schema.const !== undefined) return JSON.stringify(schema.const)
	const base = Array.isArray(schema.type) ? schema.type.join(' | ') : (schema.type ?? 'any')
	if (base === 'array' && schema.items) return `array<${typeLabel(schema.items)}>`
	return schema.format ? `${base} (${schema.format})` : base
}

function constraintNotes(schema: SchemaObject): string {
	const notes: string[] = []
	if (schema.enum) notes.push(`One of: ${schema.enum.map((value) => `\`${value}\``).join(', ')}.`)
	if (schema.pattern) notes.push(`Pattern \`${schema.pattern}\`.`)
	if (schema.minLength !== undefined && schema.minLength === schema.maxLength) notes.push(`Exactly ${schema.minLength} characters.`)
	if (schema.default !== undefined) notes.push(`Default ${JSON.stringify(schema.default)}.`)
	return notes.join(' ')
}

export type SchemaRow = {
	name: string
	depth: number
	type: string
	required: boolean
	description: string
}

function schemaRows(schema: SchemaObject, depth = 0, seen: readonly string[] = []): SchemaRow[] {
	const ref = schema.$ref
	if (ref && seen.includes(ref)) return []
	const resolved = resolve(schema)
	const rows: SchemaRow[] = []
	const required = resolved.required ?? []
	for (const [name, property] of Object.entries(resolved.properties ?? {})) {
		if (property['x-docs-hidden']) continue
		const target = resolve(property)
		const description = [singleLine(property.description ?? target.description), constraintNotes(target)]
			.filter(Boolean)
			.join(' ')
		rows.push({ name, depth, type: typeLabel(property), required: required.includes(name), description })
		if (target.properties) {
			rows.push(...schemaRows(property, depth + 1, ref ? [...seen, ref] : seen))
		}
	}
	return rows
}

export type SchemaDoc = {
	label: string
	description: string
	rows: SchemaRow[]
}

function schemaDoc(schema: SchemaObject | undefined): SchemaDoc | undefined {
	if (!schema) return undefined
	const resolved = resolve(schema)
	return {
		label: schema.$ref ? refName(schema.$ref) : typeLabel(resolved),
		description: singleLine(resolved.description),
		rows: schemaRows(schema),
	}
}

export type ParamRow = {
	name: string
	type: string
	description: string
}

export type ResponseDoc = {
	status: string
	description: string
	schema?: SchemaDoc
	retryAfter: boolean
}

export type OperationDoc = {
	method: string
	path: string
	anchor: string
	summary: string
	description: string
	params: ParamRow[]
	request?: SchemaDoc
	responses: ResponseDoc[]
}

function paramRows(parameters: ParameterObject[] | undefined): ParamRow[] {
	return (parameters ?? []).map((parameter) => {
		const resolved = resolve(parameter)
		const schema = resolved.schema ? resolve(resolved.schema) : undefined
		return {
			name: resolved.name ?? '',
			type: schema ? typeLabel(schema) : 'string',
			description: [singleLine(resolved.description), schema ? constraintNotes(schema) : '']
				.filter(Boolean)
				.join(' '),
		}
	})
}

function responseDocs(responses: Record<string, ResponseObject> | undefined): ResponseDoc[] {
	return Object.entries(responses ?? {}).map(([status, response]) => {
		const resolved = resolve(response)
		return {
			status,
			description: singleLine(resolved.description),
			schema: schemaDoc(resolved.content?.['application/json']?.schema),
			retryAfter: Boolean(resolved.headers && 'Retry-After' in resolved.headers),
		}
	})
}

const METHODS = ['get', 'post', 'put', 'delete', 'patch'] as const

export const operations: OperationDoc[] = Object.entries(spec.paths).flatMap(([path, item]) =>
	METHODS.flatMap((method) => {
		const operation = item[method]
		if (!operation) return []
		return [
			{
				method: method.toUpperCase(),
				path,
				anchor: `${method}-${path.replace(/[^a-z0-9]+/gi, '-').replace(/^-|-$/g, '')}`,
				summary: singleLine(operation.summary),
				description: singleLine(operation.description),
				params: paramRows([...(item.parameters ?? []), ...(operation.parameters ?? [])]),
				request: schemaDoc(operation.requestBody?.content?.['application/json']?.schema),
				responses: responseDocs(operation.responses),
			},
		]
	}),
)

export type ErrorCodeRow = {
	code: string
	status: string
	meaning: string
}

const ERROR_STATUS: Record<string, { status: string; meaning: string }> = {
	invalid_request: { status: '400', meaning: 'The request violates the API schema.' },
	invalid_json: { status: '400', meaning: 'The body is not valid JSON matching the API schema.' },
	payload_too_large: { status: '413', meaning: 'The body exceeds the instance size limit.' },
	invalid_id: { status: '400', meaning: 'The note ID is not 32 base62 characters.' },
	invalid_lifecycle: { status: '400', meaning: 'The lifecycle is out of range or does not match the reservation.' },
	invalid_envelope: { status: '400', meaning: 'The envelope is not canonical protocol v1 data.' },
	invalid_delete_token: { status: '403', meaning: 'The delete capability is missing or wrong.' },
	invalid_short_code: { status: '400', meaning: 'The short code is not 6 base62 characters.' },
	reservation_not_found: { status: '404', meaning: 'The reservation expired or never existed; reserve again.' },
	reservation_mismatch: { status: '409', meaning: 'The commit does not match the reservation, or the note already exists.' },
	note_not_found: { status: '404', meaning: 'The note expired, was consumed, or never existed.' },
	route_not_found: { status: '404', meaning: 'No such API route.' },
	method_not_allowed: { status: '405', meaning: 'The route exists but not for this HTTP method.' },
	rate_limited: { status: '429', meaning: 'Too many requests or hourly upload bytes; honor the Retry-After header.' },
	short_link_requires_password: { status: '409', meaning: 'Short links exist only for password-protected notes.' },
	short_code_space_exhausted: { status: '503', meaning: 'No short code could be allocated; retry later.' },
	id_space_exhausted: { status: '503', meaning: 'No note ID could be reserved; retry later.' },
	storage_unavailable: { status: '503', meaning: 'The backing store is temporarily unreachable.' },
	storage_pressure: { status: '507', meaning: 'The instance is near its storage budget; retry later or with a smaller note.' },
	writes_disabled: { status: '503', meaning: 'The operator has temporarily paused note creation.' },
	short_disabled: { status: '503', meaning: 'The operator has temporarily disabled short links.' },
	internal_error: { status: '500', meaning: 'Unexpected server-side failure.' },
}

export const errorCodes: ErrorCodeRow[] = (
	(spec.components?.schemas?.Error?.properties?.code?.enum ?? []) as string[]
).map((code) => ({
	code,
	status: ERROR_STATUS[code]?.status ?? '—',
	meaning: ERROR_STATUS[code]?.meaning ?? '',
}))

export const apiInfo = {
	title: spec.info.title,
	version: spec.info.version,
	summary: singleLine(spec.info.summary),
}
