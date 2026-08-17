export const PROTOCOL_VERSION = 1 as const
export const SECRET_BYTES = 32
export const DELETE_TOKEN_BYTES = 32
export const ID_LENGTH = 32
export const SHORT_CODE_LENGTH = 6
export const PBKDF2_ITERATIONS = 600_000
export const PBKDF2_SALT_BYTES = 16
export const AES_GCM_IV_BYTES = 12
export const AES_GCM_TAG_BYTES = 16
export const ENVELOPE_HEADER_BYTES = 73
export const MAX_HEADER_READS = 0xffff_ffff

const BASE64URL = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_'
const ID_PATTERN = /^[A-Za-z0-9]{32}$/
const HEX_HASH_PATTERN = /^[0-9a-f]{64}$/
const SHORT_CODE_PATTERN = /^[0-9]{6}$/
const encoder = new TextEncoder()
const decoder = new TextDecoder('utf-8', { fatal: true })
const KEY_DOMAIN = encoder.encode('nyanbin-v1\0')
const KEY_DOMAIN_PASSWORD = encoder.encode('nyanbin-v1-password\0')

export type NyanbinErrorCode =
  | 'INVALID_BASE64URL'
  | 'INVALID_ID'
  | 'INVALID_LINK'
  | 'INVALID_LIFECYCLE'
  | 'INVALID_PAYLOAD'
  | 'INVALID_ENVELOPE'
  | 'INVALID_DELETE_TOKEN'
  | 'AUTHENTICATION_FAILED'
  | 'INVALID_RESPONSE'
  | 'NETWORK_ERROR'
  | 'API_ERROR'

export class NyanbinError extends Error {
  readonly code: NyanbinErrorCode | (string & {})
  readonly status?: number

  constructor(code: NyanbinErrorCode | (string & {}), message: string, options?: { cause?: unknown; status?: number }) {
    super(message, options?.cause === undefined ? undefined : { cause: options.cause })
    this.name = 'NyanbinError'
    this.code = code
    this.status = options?.status
  }
}

export type Lifecycle = {
  expiresAt: number
  maxReads?: number
}

export type PrivateFile = {
  name: string
  type: string
  size: number
  data: string
}

export type PrivatePayload = {
  kind: 'text'
  format: 'plain' | 'source' | 'markdown'
  text: string
  files: PrivateFile[]
}

export type ParsedEnvelope = {
  protocol: typeof PROTOCOL_VERSION
  id: string
  lifecycle: Lifecycle
  salt: Uint8Array<ArrayBuffer>
  iv: Uint8Array<ArrayBuffer>
  ciphertext: Uint8Array<ArrayBuffer>
}

export type EncryptOptions = {
  id: string
  lifecycle: Lifecycle
  secret?: Uint8Array | string
  password?: string
}

export type DecryptOptions = Omit<EncryptOptions, 'lifecycle'> & {
  lifecycle?: { expiresAt: number; maxReads?: number; remainingReads?: number }
}

export type NyanbinLink = {
  server: string
  id: string
  secret: Uint8Array
}
export type NyanbinReference = Omit<NyanbinLink, 'secret'> & { secret?: Uint8Array }

function fail(code: NyanbinErrorCode, message: string): never {
  throw new NyanbinError(code, message)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasOnlyKeys(value: Record<string, unknown>, required: readonly string[], optional: readonly string[] = []): boolean {
  const allowed = new Set([...required, ...optional])
  return required.every((key) => Object.hasOwn(value, key)) && Object.keys(value).every((key) => allowed.has(key))
}

function concatBytes(...parts: Uint8Array[]): Uint8Array<ArrayBuffer> {
  const result = new Uint8Array(parts.reduce((total, part) => total + part.length, 0))
  let offset = 0
  for (const part of parts) {
    result.set(part, offset)
    offset += part.length
  }
  return result
}

export function encodeBase64Url(data: Uint8Array): string {
  let result = ''
  for (let i = 0; i < data.length; i += 3) {
    const a = data[i]!
    const b = data[i + 1]
    const c = data[i + 2]
    const value = (a << 16) | ((b ?? 0) << 8) | (c ?? 0)
    result += BASE64URL[(value >>> 18) & 63]
    result += BASE64URL[(value >>> 12) & 63]
    if (b !== undefined) result += BASE64URL[(value >>> 6) & 63]
    if (c !== undefined) result += BASE64URL[value & 63]
  }
  return result
}

export function decodeBase64Url(value: string, options: { length?: number; label?: string } = {}): Uint8Array<ArrayBuffer> {
  const label = options.label ?? 'value'
  if (typeof value !== 'string' || value.length % 4 === 1 || !/^[A-Za-z0-9_-]*$/.test(value)) {
    fail('INVALID_BASE64URL', `${label} must be canonical unpadded base64url`)
  }
  const output = new Uint8Array(Math.floor((value.length * 6) / 8))
  let bits = 0
  let accumulator = 0
  let offset = 0
  for (const character of value) {
    accumulator = (accumulator << 6) | BASE64URL.indexOf(character)
    bits += 6
    if (bits >= 8) {
      bits -= 8
      output[offset++] = (accumulator >>> bits) & 0xff
      accumulator &= (1 << bits) - 1
    }
  }
  if (bits !== 0 && accumulator !== 0) fail('INVALID_BASE64URL', `${label} has non-zero trailing bits`)
  if (encodeBase64Url(output) !== value) fail('INVALID_BASE64URL', `${label} must be canonical unpadded base64url`)
  if (options.length !== undefined && output.length !== options.length) {
    fail('INVALID_BASE64URL', `${label} must decode to ${options.length} bytes`)
  }
  return output
}

export function validateId(id: unknown): asserts id is string {
  if (typeof id !== 'string' || !ID_PATTERN.test(id)) fail('INVALID_ID', `note id must be ${ID_LENGTH} base62 characters`)
}

export function validateShortCode(code: unknown): asserts code is string {
  if (typeof code !== 'string' || !SHORT_CODE_PATTERN.test(code)) {
    fail('INVALID_LINK', `short code must be ${SHORT_CODE_LENGTH} digits`)
  }
}

export function validateLifecycle(value: unknown): asserts value is Lifecycle {
  if (!isRecord(value) || !hasOnlyKeys(value, ['expiresAt'], ['maxReads'])) fail('INVALID_LIFECYCLE', 'lifecycle has an invalid shape')
  if (!Number.isSafeInteger(value.expiresAt) || (value.expiresAt as number) <= 0) {
    fail('INVALID_LIFECYCLE', 'expiresAt must be a positive integer Unix timestamp in milliseconds')
  }
  if (
    value.maxReads !== undefined &&
    (!Number.isSafeInteger(value.maxReads) || (value.maxReads as number) <= 0 || (value.maxReads as number) > MAX_HEADER_READS)
  ) {
    fail('INVALID_LIFECYCLE', `maxReads must be an integer between 1 and ${MAX_HEADER_READS}`)
  }
}

export function validateDeleteTokenHash(hash: unknown): asserts hash is string {
  if (typeof hash !== 'string' || !HEX_HASH_PATTERN.test(hash)) {
    fail('INVALID_DELETE_TOKEN', 'delete token hash must be 64 lowercase hexadecimal characters')
  }
}

const PORTABLE_FILENAME_BYTES = 255
const WINDOWS_DEVICE_BASENAME = /^(?:CON|PRN|AUX|NUL|CLOCK\$|CONIN\$|CONOUT\$|COM[1-9¹²³]|LPT[1-9¹²³])[ .]*(?:\.|$)/i
const TERMINAL_OR_BIDI_CONTROL = /[\u0000-\u001f\u007f-\u009f\u061c\u200e\u200f\u2028-\u202e\u2066-\u2069]/u
const MIME_TYPE = /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+\/[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/

export function validatePortableFilename(name: unknown): asserts name is string {
  if (
    typeof name !== 'string' ||
    name.length === 0 ||
    encoder.encode(name).length > PORTABLE_FILENAME_BYTES ||
    TERMINAL_OR_BIDI_CONTROL.test(name) ||
    /[\\/:]/.test(name) ||
    /^[ .]|[ .]$/.test(name) ||
    WINDOWS_DEVICE_BASENAME.test(name)
  ) {
    fail('INVALID_PAYLOAD', 'private file name is not portable')
  }
}

export function validateMimeType(type: unknown): asserts type is string {
  if (typeof type !== 'string' || type.length > 255 || !MIME_TYPE.test(type)) {
    fail('INVALID_PAYLOAD', 'private file MIME type is invalid')
  }
}

export function validatePayload(value: unknown): asserts value is PrivatePayload {
  if (!isRecord(value) || !hasOnlyKeys(value, ['kind', 'format', 'text', 'files'])) fail('INVALID_PAYLOAD', 'private payload has an invalid shape')
  if (value.kind !== 'text') fail('INVALID_PAYLOAD', 'private payload kind must be text')
  if (value.format !== 'plain' && value.format !== 'source' && value.format !== 'markdown') {
    fail('INVALID_PAYLOAD', 'private payload format is invalid')
  }
  if (typeof value.text !== 'string' || !Array.isArray(value.files)) fail('INVALID_PAYLOAD', 'private payload text or files are invalid')
  const names = new Set<string>()
  for (const file of value.files) {
    if (!isRecord(file) || !hasOnlyKeys(file, ['name', 'type', 'size', 'data'])) fail('INVALID_PAYLOAD', 'private file has an invalid shape')
    validatePortableFilename(file.name)
    if (names.has(file.name)) fail('INVALID_PAYLOAD', 'private file names must be unique')
    names.add(file.name)
    validateMimeType(file.type)
    if (!Number.isSafeInteger(file.size) || (file.size as number) < 0) fail('INVALID_PAYLOAD', 'private file size is invalid')
    if (typeof file.data !== 'string') fail('INVALID_PAYLOAD', 'private file data is invalid')
    const bytes = decodeBase64Url(file.data, { label: `data for ${file.name}` })
    if (bytes.length !== file.size) fail('INVALID_PAYLOAD', `private file size does not match data for ${file.name}`)
  }
}

export function canonicalAadString(id: string, lifecycle: Lifecycle): string {
  validateId(id)
  validateLifecycle(lifecycle)
  return `{"protocol":${PROTOCOL_VERSION},"id":${JSON.stringify(id)},"expiresAt":${lifecycle.expiresAt},"maxReads":${lifecycle.maxReads ?? 'null'}}`
}

export function canonicalAad(id: string, lifecycle: Lifecycle): Uint8Array<ArrayBuffer> {
  return encoder.encode(canonicalAadString(id, lifecycle))
}

export function generateSecret(): Uint8Array<ArrayBuffer> {
  const secret = new Uint8Array(SECRET_BYTES)
  globalThis.crypto.getRandomValues(secret)
  return secret
}

function normalizeSecret(secret: Uint8Array | string): Uint8Array<ArrayBuffer> {
  const bytes = typeof secret === 'string' ? decodeBase64Url(secret, { length: SECRET_BYTES, label: 'link secret' }) : secret
  if (!(bytes instanceof Uint8Array) || bytes.length !== SECRET_BYTES) fail('INVALID_LINK', `link secret must contain ${SECRET_BYTES} bytes`)
  return bytes.buffer instanceof ArrayBuffer
    ? new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength)
    : new Uint8Array(bytes)
}
async function passwordFactor(password: string, salt: Uint8Array<ArrayBuffer>): Promise<Uint8Array<ArrayBuffer>> {
  const material = await globalThis.crypto.subtle.importKey('raw', encoder.encode(password), 'PBKDF2', false, ['deriveBits'])
  return new Uint8Array(
    await globalThis.crypto.subtle.deriveBits({ name: 'PBKDF2', hash: 'SHA-256', salt, iterations: PBKDF2_ITERATIONS }, material, 256)
  )
}

async function importAesKey(preimage: Uint8Array<ArrayBuffer>): Promise<CryptoKey> {
  const rawKey = await globalThis.crypto.subtle.digest('SHA-256', preimage)
  return globalThis.crypto.subtle.importKey('raw', rawKey, { name: 'AES-GCM' }, false, ['encrypt', 'decrypt'])
}

// Password-protected notes are keyed by the password alone, so bare links (no #fragment) can decrypt.
async function derivePasswordKey(password: string, salt: Uint8Array<ArrayBuffer>): Promise<CryptoKey> {
  return importAesKey(concatBytes(KEY_DOMAIN_PASSWORD, await passwordFactor(password, salt)))
}

// v1.4.0 envelopes mixed the link secret with the (possibly empty) password; kept so outstanding links decrypt.
async function deriveSecretKey(secret: Uint8Array<ArrayBuffer>, password: string, salt: Uint8Array<ArrayBuffer>): Promise<CryptoKey> {
  return importAesKey(concatBytes(KEY_DOMAIN, secret, await passwordFactor(password, salt)))
}

export function parseEnvelope(envelope: string): ParsedEnvelope {
  const bytes = decodeBase64Url(envelope, { label: 'envelope' })
  if (bytes.length < ENVELOPE_HEADER_BYTES + AES_GCM_TAG_BYTES) fail('INVALID_ENVELOPE', 'encrypted envelope is too short')
  if (bytes[0] !== PROTOCOL_VERSION) fail('INVALID_ENVELOPE', 'encrypted envelope protocol is unsupported')
  let id: string
  try {
    id = decoder.decode(bytes.subarray(1, 33))
  } catch (cause) {
    throw new NyanbinError('INVALID_ENVELOPE', 'encrypted envelope id is not ASCII', { cause })
  }
  validateId(id)
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const expiresAtBigInt = view.getBigUint64(33, false)
  if (expiresAtBigInt > BigInt(Number.MAX_SAFE_INTEGER)) fail('INVALID_ENVELOPE', 'encrypted envelope expiry is out of range')
  const encodedMaxReads = view.getUint32(41, false)
  const lifecycle: Lifecycle = {
    expiresAt: Number(expiresAtBigInt),
    ...(encodedMaxReads === 0 ? {} : { maxReads: encodedMaxReads }),
  }
  validateLifecycle(lifecycle)
  return {
    protocol: PROTOCOL_VERSION,
    id,
    lifecycle,
    salt: bytes.slice(45, 61),
    iv: bytes.slice(61, ENVELOPE_HEADER_BYTES),
    ciphertext: bytes.slice(ENVELOPE_HEADER_BYTES),
  }
}

export async function encryptPayload(payload: PrivatePayload, options: EncryptOptions): Promise<string> {
  validatePayload(payload)
  validateId(options.id)
  validateLifecycle(options.lifecycle)
  const password = options.password ?? ''
  if (password === '' && options.secret === undefined) fail('INVALID_LINK', 'a link secret is required when no password is set')
  const salt = new Uint8Array(PBKDF2_SALT_BYTES)
  const iv = new Uint8Array(AES_GCM_IV_BYTES)
  globalThis.crypto.getRandomValues(salt)
  globalThis.crypto.getRandomValues(iv)
  const key = password === '' ? await deriveSecretKey(normalizeSecret(options.secret!), '', salt) : await derivePasswordKey(password, salt)
  const plaintext = encoder.encode(JSON.stringify(payload))
  const ciphertext = new Uint8Array(
    await globalThis.crypto.subtle.encrypt(
      { name: 'AES-GCM', iv, additionalData: canonicalAad(options.id, options.lifecycle), tagLength: AES_GCM_TAG_BYTES * 8 },
      key,
      plaintext
    )
  )
  const header = new Uint8Array(ENVELOPE_HEADER_BYTES)
  header[0] = PROTOCOL_VERSION
  header.set(encoder.encode(options.id), 1)
  const headerView = new DataView(header.buffer)
  headerView.setBigUint64(33, BigInt(options.lifecycle.expiresAt), false)
  headerView.setUint32(41, options.lifecycle.maxReads ?? 0, false)
  header.set(salt, 45)
  header.set(iv, 61)
  return encodeBase64Url(concatBytes(header, ciphertext))
}

export async function decryptPayload(envelope: string, options: DecryptOptions): Promise<PrivatePayload> {
  validateId(options.id)
  const parsed = parseEnvelope(envelope)
  if (parsed.id !== options.id) fail('INVALID_ENVELOPE', 'encrypted envelope id does not match the note link')
  if (options.lifecycle !== undefined) {
    if (!Number.isSafeInteger(options.lifecycle.expiresAt) || options.lifecycle.expiresAt <= 0) {
      fail('INVALID_LIFECYCLE', 'note information expiry is invalid')
    }
    if (parsed.lifecycle.expiresAt !== options.lifecycle.expiresAt) {
      fail('INVALID_ENVELOPE', 'encrypted envelope expiry does not match note information')
    }
    if (options.lifecycle.maxReads !== undefined && parsed.lifecycle.maxReads !== options.lifecycle.maxReads) {
      fail('INVALID_ENVELOPE', 'encrypted envelope read limit does not match note information')
    }
  }
  const password = options.password ?? ''
  const secret = options.secret === undefined ? undefined : normalizeSecret(options.secret)
  if (password === '' && secret === undefined) fail('INVALID_LINK', 'a link secret or password is required to decrypt')
  const candidates: Promise<CryptoKey>[] = []
  if (password !== '') candidates.push(derivePasswordKey(password, parsed.salt))
  // Legacy v1.4.0 path: link secret mixed with the (possibly empty) password.
  if (secret !== undefined) candidates.push(deriveSecretKey(secret, password, parsed.salt))
  let plaintext: ArrayBuffer | undefined
  let lastCause: unknown
  for (const candidate of candidates) {
    try {
      plaintext = await globalThis.crypto.subtle.decrypt(
        {
          name: 'AES-GCM',
          iv: parsed.iv,
          additionalData: canonicalAad(parsed.id, parsed.lifecycle),
          tagLength: AES_GCM_TAG_BYTES * 8,
        },
        await candidate,
        parsed.ciphertext
      )
      break
    } catch (cause) {
      lastCause = cause
    }
  }
  if (plaintext === undefined) {
    throw new NyanbinError('AUTHENTICATION_FAILED', 'envelope authentication failed; the link, password, or ciphertext is incorrect', { cause: lastCause })
  }
  let value: unknown
  try {
    value = JSON.parse(decoder.decode(plaintext))
  } catch (cause) {
    throw new NyanbinError('INVALID_PAYLOAD', 'decrypted private payload is not valid UTF-8 JSON', { cause })
  }
  validatePayload(value)
  return value
}

export async function hashDeleteToken(token: string): Promise<string> {
  const bytes = decodeBase64Url(token, { length: DELETE_TOKEN_BYTES, label: 'delete token' })
  const digest = new Uint8Array(await globalThis.crypto.subtle.digest('SHA-256', bytes))
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

function serverOrigin(server: string): URL {
  let url: URL
  try {
    url = new URL(server)
  } catch (cause) {
    throw new NyanbinError('INVALID_LINK', 'server must be an absolute HTTP(S) URL', { cause })
  }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') fail('INVALID_LINK', 'server must use HTTP or HTTPS')
  if (url.username || url.password || url.search || url.hash || (url.pathname !== '' && url.pathname !== '/')) {
    fail('INVALID_LINK', 'server URL must be a bare HTTP(S) origin')
  }
  return url
}

export function buildNoteUrl(server: string, id: string, secret?: Uint8Array | string): string {
  validateId(id)
  const url = serverOrigin(server)
  url.pathname = `/note/${id}`
  if (secret !== undefined) url.hash = encodeBase64Url(normalizeSecret(secret))
  return url.toString()
}

export function buildShortUrl(server: string, code: string): string {
  validateShortCode(code)
  const url = serverOrigin(server)
  url.pathname = `/s/${code}`
  return url.toString()
}

export function parseNoteReference(input: string): NyanbinReference {
  let url: URL
  try {
    url = new URL(input)
  } catch (cause) {
    throw new NyanbinError('INVALID_LINK', 'note link must be an absolute URL', { cause })
  }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') fail('INVALID_LINK', 'note link must use HTTP or HTTPS')
  if (url.username || url.password || url.search) fail('INVALID_LINK', 'note link must not contain credentials or a query string')
  const match = /^\/note\/([A-Za-z0-9]{32})\/?$/.exec(url.pathname)
  if (!match) fail('INVALID_LINK', 'note link path must be /note/{id}')
  const id = match[1]!
  validateId(id)
  const secret = url.hash === '' ? undefined : decodeBase64Url(url.hash.slice(1), { length: SECRET_BYTES, label: 'link secret' })
  return { server: url.origin, id, ...(secret === undefined ? {} : { secret }) }
}

export function parseNoteLink(input: string): NyanbinLink {
  const reference = parseNoteReference(input)
  if (reference.secret === undefined) fail('INVALID_LINK', 'note link is missing its secret fragment')
  return { ...reference, secret: reference.secret }
}
