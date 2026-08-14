import {
  DELETE_TOKEN_BYTES,
  NyanbinError,
  PROTOCOL_VERSION,
  decodeBase64Url,
  parseEnvelope,
  validateDeleteTokenHash,
  validateId,
  validateLifecycle,
  type Lifecycle,
} from './protocol.js'

export type ReserveRequest = {
  expiresIn: number
  maxReads?: number
}

export type ReserveResponse = {
  id: string
  deleteToken: string
  lifecycle: Lifecycle
}

export type CreateRequest = {
  protocol: typeof PROTOCOL_VERSION
  envelope: string
  lifecycle: Lifecycle
  deleteTokenHash: string
}

export type CreateResponse = {
  id: string
}

export type InfoLifecycle = {
  expiresAt: number
  maxReads?: number
  remainingReads?: number
}

export type NoteInfo = {
  protocol: typeof PROTOCOL_VERSION
  lifecycle: InfoLifecycle
}

export type RevealResponse = {
  protocol: typeof PROTOCOL_VERSION
  envelope: string
}

export type DeleteRequest = {
  deleteToken: string
}

export type Status = {
  protocol: typeof PROTOCOL_VERSION
  version: string
  limits: {
    maxEnvelopeBytes: number
    maxExpiresIn: number
    maxReads: number
  }
  defaults: {
    expiresIn: number
    maxReads?: number
  }
  capabilities: {
    files: boolean
    passwords: boolean
    formats: Array<'plain' | 'source' | 'markdown'>
  }
  branding: {
    name: string
    description: string
    logoUrl: string
    imprintUrl: string
  }
}

export type ClientOptions = {
  readonly server: string
  readonly fetch: typeof globalThis.fetch
}

export type APIClient = {
  readonly server: string
  readonly reserve: (request: ReserveRequest) => Promise<ReserveResponse>
  readonly commit: (id: string, request: CreateRequest) => Promise<CreateResponse>
  readonly info: (id: string) => Promise<NoteInfo>
  readonly reveal: (id: string) => Promise<RevealResponse>
  readonly delete: (id: string, deleteToken: string) => Promise<void>
  readonly deleteNote: (id: string, deleteToken: string) => Promise<void>
  readonly status: () => Promise<Status>
}

type CallOptions = {
  path: string
  method: 'GET' | 'POST' | 'PUT' | 'DELETE'
  body?: unknown
  empty?: boolean
}

function objectAtBoundary(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new NyanbinError('INVALID_RESPONSE', `${label} is not a JSON object`)
  }
  return value as Record<string, unknown>
}

function exactKeys(value: Record<string, unknown>, required: readonly string[], optional: readonly string[] = []): void {
  const allowed = new Set([...required, ...optional])
  if (!required.every((key) => Object.hasOwn(value, key)) || !Object.keys(value).every((key) => allowed.has(key))) {
    throw new NyanbinError('INVALID_RESPONSE', 'server response contains missing or unknown fields')
  }
}

function validateProtocol(value: unknown): asserts value is typeof PROTOCOL_VERSION {
  if (value !== PROTOCOL_VERSION) throw new NyanbinError('INVALID_RESPONSE', 'server protocol is unsupported')
}

function validatePositiveInteger(value: unknown, label: string, allowZero = false): asserts value is number {
  if (!Number.isSafeInteger(value) || (allowZero ? (value as number) < 0 : (value as number) <= 0)) {
    throw new NyanbinError('INVALID_RESPONSE', `${label} must be ${allowZero ? 'a non-negative' : 'a positive'} integer`)
  }
}

function validateString(value: unknown, label: string): asserts value is string {
  if (typeof value !== 'string') throw new NyanbinError('INVALID_RESPONSE', `${label} must be a string`)
}

function normalizeOptions(options: Partial<ClientOptions>): Readonly<ClientOptions> {
  let server = options.server?.replace(/\/$/, '') ?? ''
  if (server !== '') {
    let parsed: URL
    try {
      parsed = new URL(server)
    } catch (cause) {
      throw new NyanbinError('INVALID_LINK', 'server must be an absolute HTTP(S) URL', { cause })
    }
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
      throw new NyanbinError('INVALID_LINK', 'server must use HTTP or HTTPS')
    }
    if (parsed.username || parsed.password || parsed.search || parsed.hash || (parsed.pathname !== '' && parsed.pathname !== '/')) {
      throw new NyanbinError('INVALID_LINK', 'server URL must be a bare HTTP(S) origin')
    }
    server = parsed.origin
  }
  return Object.freeze({
    server,
    fetch: options.fetch ?? globalThis.fetch.bind(globalThis),
  })
}

async function call(client: Readonly<ClientOptions>, options: CallOptions): Promise<unknown> {
  let response: Response
  try {
    response = await client.fetch(`${client.server}/api${options.path}`, {
      method: options.method,
      body: options.body === undefined ? undefined : JSON.stringify(options.body),
      mode: 'cors',
      headers: options.body === undefined ? { Accept: 'application/json' } : { Accept: 'application/json', 'Content-Type': 'application/json' },
    })
  } catch (cause) {
    throw new NyanbinError('NETWORK_ERROR', 'could not reach the Nyanbin server', { cause })
  }

  if (!response.ok) {
    let code = 'API_ERROR'
    let message = `Nyanbin server returned HTTP ${response.status}`
    try {
      const error = objectAtBoundary(await response.json(), 'error response')
      if (typeof error.code === 'string' && typeof error.message === 'string') {
        code = error.code
        message = error.message
      }
    } catch (cause) {
      if (cause instanceof NyanbinError && cause.code !== 'INVALID_RESPONSE') throw cause
    }
    throw new NyanbinError(code, message, { status: response.status })
  }

  if (options.empty || response.status === 204) return undefined
  try {
    return await response.json()
  } catch (cause) {
    throw new NyanbinError('INVALID_RESPONSE', 'server returned invalid JSON', { cause, status: response.status })
  }
}

async function reserve(client: Readonly<ClientOptions>, request: ReserveRequest): Promise<ReserveResponse> {
  validatePositiveInteger(request.expiresIn, 'expiresIn')
  if (request.maxReads !== undefined) validatePositiveInteger(request.maxReads, 'maxReads', true)
  if (!Object.keys(request).every((key) => key === 'expiresIn' || key === 'maxReads')) {
    throw new NyanbinError('INVALID_LIFECYCLE', 'reserve request contains unknown fields')
  }
  const data = objectAtBoundary(await call(client, { path: '/notes/reserve', method: 'POST', body: request }), 'reserve response')
  exactKeys(data, ['id', 'deleteToken', 'lifecycle'])
  validateId(data.id)
  if (typeof data.deleteToken !== 'string') throw new NyanbinError('INVALID_RESPONSE', 'deleteToken must be a string')
  decodeBase64Url(data.deleteToken, { length: DELETE_TOKEN_BYTES, label: 'delete token' })
  validateLifecycle(data.lifecycle)
  return { id: data.id, deleteToken: data.deleteToken, lifecycle: data.lifecycle }
}

async function commit(client: Readonly<ClientOptions>, id: string, request: CreateRequest): Promise<CreateResponse> {
  validateId(id)
  if (request.protocol !== PROTOCOL_VERSION) throw new NyanbinError('INVALID_ENVELOPE', 'create protocol must be 1')
  validateLifecycle(request.lifecycle)
  validateDeleteTokenHash(request.deleteTokenHash)
  if (!Object.keys(request).every((key) => key === 'protocol' || key === 'envelope' || key === 'lifecycle' || key === 'deleteTokenHash')) {
    throw new NyanbinError('INVALID_ENVELOPE', 'create request contains unknown fields')
  }
  const header = parseEnvelope(request.envelope)
  if (
    header.id !== id ||
    header.lifecycle.expiresAt !== request.lifecycle.expiresAt ||
    header.lifecycle.maxReads !== request.lifecycle.maxReads
  ) throw new NyanbinError('INVALID_ENVELOPE', 'envelope header does not match the reserved note lifecycle')
  const data = objectAtBoundary(await call(client, { path: `/notes/${id}`, method: 'PUT', body: request }), 'create response')
  exactKeys(data, ['id'])
  validateId(data.id)
  if (data.id !== id) throw new NyanbinError('INVALID_RESPONSE', 'created note id does not match reservation')
  return { id: data.id }
}

async function info(client: Readonly<ClientOptions>, id: string): Promise<NoteInfo> {
  validateId(id)
  const data = objectAtBoundary(await call(client, { path: `/notes/${id}`, method: 'GET' }), 'note info')
  exactKeys(data, ['protocol', 'lifecycle'])
  validateProtocol(data.protocol)
  const lifecycle = objectAtBoundary(data.lifecycle, 'note lifecycle')
  exactKeys(lifecycle, ['expiresAt'], ['maxReads', 'remainingReads'])
  validatePositiveInteger(lifecycle.expiresAt, 'expiresAt')
  if (lifecycle.maxReads !== undefined) validatePositiveInteger(lifecycle.maxReads, 'maxReads')
  if (lifecycle.remainingReads !== undefined) validatePositiveInteger(lifecycle.remainingReads, 'remainingReads', true)
  return {
    protocol: PROTOCOL_VERSION,
    lifecycle: {
      expiresAt: lifecycle.expiresAt,
      ...(lifecycle.maxReads === undefined ? {} : { maxReads: lifecycle.maxReads }),
      ...(lifecycle.remainingReads === undefined ? {} : { remainingReads: lifecycle.remainingReads }),
    },
  }
}

async function reveal(client: Readonly<ClientOptions>, id: string): Promise<RevealResponse> {
  validateId(id)
  const data = objectAtBoundary(await call(client, { path: `/notes/${id}/reveal`, method: 'POST' }), 'reveal response')
  exactKeys(data, ['protocol', 'envelope'])
  validateProtocol(data.protocol)
  validateString(data.envelope, 'envelope')
  const header = parseEnvelope(data.envelope)
  if (header.id !== id) throw new NyanbinError('INVALID_ENVELOPE', 'revealed envelope does not match note id')
  return { protocol: PROTOCOL_VERSION, envelope: data.envelope }
}

async function removeNote(client: Readonly<ClientOptions>, id: string, deleteToken: string): Promise<void> {
  validateId(id)
  decodeBase64Url(deleteToken, { length: DELETE_TOKEN_BYTES, label: 'delete token' })
  const body: DeleteRequest = { deleteToken }
  await call(client, { path: `/notes/${id}`, method: 'DELETE', body, empty: true })
}

async function status(client: Readonly<ClientOptions>): Promise<Status> {
  const data = objectAtBoundary(await call(client, { path: '/status', method: 'GET' }), 'status response')
  exactKeys(data, ['protocol', 'version', 'limits', 'defaults', 'capabilities', 'branding'])
  validateProtocol(data.protocol)
  validateString(data.version, 'instance version')
  const limits = objectAtBoundary(data.limits, 'status limits')
  exactKeys(limits, ['maxEnvelopeBytes', 'maxExpiresIn', 'maxReads'])
  validatePositiveInteger(limits.maxEnvelopeBytes, 'maxEnvelopeBytes')
  validatePositiveInteger(limits.maxExpiresIn, 'maxExpiresIn')
  validatePositiveInteger(limits.maxReads, 'maxReads')
  const defaults = objectAtBoundary(data.defaults, 'status defaults')
  exactKeys(defaults, ['expiresIn'], ['maxReads'])
  validatePositiveInteger(defaults.expiresIn, 'default expiresIn')
  if (defaults.maxReads !== undefined) validatePositiveInteger(defaults.maxReads, 'default maxReads')
  const capabilities = objectAtBoundary(data.capabilities, 'status capabilities')
  exactKeys(capabilities, ['files', 'passwords', 'formats'])
  if (typeof capabilities.files !== 'boolean' || typeof capabilities.passwords !== 'boolean') {
    throw new NyanbinError('INVALID_RESPONSE', 'status capabilities must be boolean')
  }
  if (
    !Array.isArray(capabilities.formats) ||
    capabilities.formats.some((format) => format !== 'plain' && format !== 'source' && format !== 'markdown')
  ) throw new NyanbinError('INVALID_RESPONSE', 'status formats are invalid')
  const branding = objectAtBoundary(data.branding, 'status branding')
  exactKeys(branding, ['name', 'description', 'logoUrl', 'imprintUrl'])
  validateString(branding.name, 'branding name')
  validateString(branding.description, 'branding description')
  validateString(branding.logoUrl, 'branding logoUrl')
  validateString(branding.imprintUrl, 'branding imprintUrl')
  return {
    protocol: PROTOCOL_VERSION,
    version: data.version,
    limits: {
      maxEnvelopeBytes: limits.maxEnvelopeBytes,
      maxExpiresIn: limits.maxExpiresIn,
      maxReads: limits.maxReads,
    },
    defaults: {
      expiresIn: defaults.expiresIn,
      ...(defaults.maxReads === undefined ? {} : { maxReads: defaults.maxReads }),
    },
    capabilities: {
      files: capabilities.files,
      passwords: capabilities.passwords,
      formats: [...capabilities.formats],
    },
    branding: {
      name: branding.name,
      description: branding.description,
      logoUrl: branding.logoUrl,
      imprintUrl: branding.imprintUrl,
    },
  }
}

class ImmutableAPIClient implements APIClient {
  readonly server: string

  constructor(private readonly client: Readonly<ClientOptions>) {
    this.server = client.server
    Object.freeze(this)
  }

  reserve(request: ReserveRequest): Promise<ReserveResponse> { return reserve(this.client, request) }
  commit(id: string, request: CreateRequest): Promise<CreateResponse> { return commit(this.client, id, request) }
  info(id: string): Promise<NoteInfo> { return info(this.client, id) }
  reveal(id: string): Promise<RevealResponse> { return reveal(this.client, id) }
  delete(id: string, deleteToken: string): Promise<void> { return removeNote(this.client, id, deleteToken) }
  deleteNote(id: string, deleteToken: string): Promise<void> { return removeNote(this.client, id, deleteToken) }
  status(): Promise<Status> { return status(this.client) }
}

Object.freeze(ImmutableAPIClient.prototype)

export function createAPI(options: Partial<ClientOptions> = {}): APIClient {
  return new ImmutableAPIClient(normalizeOptions(options))
}

export const API: APIClient = createAPI()
