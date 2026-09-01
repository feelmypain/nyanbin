import { expect, test } from '@playwright/test'
import { readFileSync } from 'node:fs'
import Ajv, { type ValidateFunction } from 'ajv'
import { parse } from 'yaml'
import { commitNote, SERVER, type ApiReservation } from '../utils'

// The API contract is browser-independent; run it once, on Chromium.
test.skip(({ browserName }) => browserName !== 'chromium', 'API contract specs run once, on Chromium')

const BASE62 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
const randomId = () =>
  Array.from({ length: 32 }, () => BASE62[Math.floor(Math.random() * BASE62.length)]).join('')

type OpenApiDocument = {
  openapi: string
  paths: Record<string, unknown>
  components: { schemas: Record<string, unknown> }
}

const ERROR_CODES = [
  'invalid_request',
  'invalid_json',
  'payload_too_large',
  'invalid_id',
  'invalid_lifecycle',
  'invalid_envelope',
  'invalid_delete_token',
  'invalid_short_code',
  'reservation_not_found',
  'reservation_mismatch',
  'note_not_found',
  'route_not_found',
  'method_not_allowed',
  'rate_limited',
  'short_link_requires_password',
  'short_code_space_exhausted',
  'id_space_exhausted',
  'storage_unavailable',
  'storage_pressure',
  'writes_disabled',
  'short_disabled',
  'internal_error',
] as const

let document: OpenApiDocument
let ajv: Ajv

test.beforeAll(async () => {
  const response = await fetch(`${SERVER}/api/openapi.json`)
  expect(response.status).toBe(200)
  document = (await response.json()) as OpenApiDocument
  ajv = new Ajv({ strict: false })
  ajv.addSchema(document as unknown as object, 'openapi')
})

function schemaValidator(name: string): ValidateFunction {
  const validator = ajv.getSchema(`openapi#/components/schemas/${name}`)
  if (!validator) throw new Error(`Schema ${name} is missing from the served OpenAPI document`)
  return validator
}

function expectValid(name: string, body: unknown): void {
  const validator = schemaValidator(name)
  const valid = validator(body)
  expect(validator.errors ?? [], `${name} must accept ${JSON.stringify(body)}`).toEqual([])
  expect(valid).toBe(true)
}

async function expectError(
  response: Response,
  status: number,
  code: (typeof ERROR_CODES)[number],
): Promise<void> {
  expect(response.status).toBe(status)
  const body = (await response.json()) as Record<string, unknown>
  expect(Object.keys(body).sort()).toEqual(['code', 'message'])
  expect(ERROR_CODES).toContain(body.code)
  expect(body.code).toBe(code)
  expectValid('Error', body)
}

test('served OpenAPI JSON matches the committed YAML source', async () => {
  const response = await fetch(`${SERVER}/api/openapi.json`)
  expect(response.status).toBe(200)
  expect(response.headers.get('content-type')).toContain('application/json')
  const served = await response.json()
  expect((served as OpenApiDocument).openapi).toBe('3.1.0')
  expect(served).toEqual(parse(readFileSync('docs/api/openapi.yaml', 'utf8')))
})

test('every response across a full note lifecycle validates against its schema', async () => {
  for (const [path, name] of [
    ['/api/status', 'Status'],
    ['/api/live', 'Health'],
    ['/api/ready', 'Health'],
  ] as const) {
    const response = await fetch(`${SERVER}${path}`)
    expect(response.status).toBe(200)
    expectValid(name, await response.json())
  }

  const reserveResponse = await fetch(`${SERVER}/api/notes/reserve`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ expiresIn: 3600, maxReads: 5 }),
  })
  expect(reserveResponse.status).toBe(201)
  const reservation = (await reserveResponse.json()) as ApiReservation
  expectValid('ReserveResponse', reservation)
  expectValid('Lifecycle', reservation.lifecycle)

  const { response: commitResponse } = await commitNote(SERVER, reservation, { textBytes: 128 })
  expect([200, 201]).toContain(commitResponse.status)
  const created = (await commitResponse.json()) as { id: string }
  expectValid('CreateResponse', created)
  expect(created.id).toBe(reservation.id)

  const infoResponse = await fetch(`${SERVER}/api/notes/${reservation.id}`)
  expect(infoResponse.status).toBe(200)
  const info = (await infoResponse.json()) as { lifecycle: unknown }
  expectValid('InfoResponse', info)
  expectValid('InfoLifecycle', info.lifecycle)

  const revealResponse = await fetch(`${SERVER}/api/notes/${reservation.id}/reveal`, { method: 'POST' })
  expect(revealResponse.status).toBe(200)
  expectValid('RevealResponse', await revealResponse.json())

  const deleteResponse = await fetch(`${SERVER}/api/notes/${reservation.id}`, {
    method: 'DELETE',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ deleteToken: reservation.deleteToken }),
  })
  expect(deleteResponse.status).toBe(204)
  expect(await deleteResponse.text()).toBe('')
})

test('error responses carry exactly {code, message} with enumerated codes', async () => {
  await expectError(await fetch(`${SERVER}/api/notes/!!!`), 400, 'invalid_id')

  const garbageCommit = await fetch(`${SERVER}/api/notes/${randomId()}`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: 'not json at all',
  })
  await expectError(garbageCommit, 400, 'invalid_json')

  const orphanReservation = {
    id: randomId(),
    deleteToken: 'A'.repeat(43),
    lifecycle: { expiresAt: Date.now() + 3_600_000, maxReads: 5 },
  }
  const { response: orphanCommit } = await commitNote(SERVER, orphanReservation, { textBytes: 64 })
  await expectError(orphanCommit, 404, 'reservation_not_found')

  await expectError(await fetch(`${SERVER}/api/notes/${randomId()}`), 404, 'note_not_found')
  await expectError(await fetch(`${SERVER}/api/short/!!!`), 400, 'invalid_short_code')
  await expectError(await fetch(`${SERVER}/api/nope`), 404, 'route_not_found')
  await expectError(
    await fetch(`${SERVER}/api/notes/reserve`, { method: 'PATCH' }),
    405,
    'method_not_allowed',
  )
})

test('the human-readable API reference documents every path in the contract', async () => {
  const response = await fetch(`${SERVER}/docs/api`)
  expect(response.status).toBe(200)
  expect(response.headers.get('content-type')).toContain('text/html')
  // The prerendered page may escape braces in path templates; normalize before matching.
  const html = (await response.text()).replaceAll('&#123;', '{').replaceAll('&#125;', '}')
  const paths = Object.keys(document.paths)
  expect(paths.length).toBeGreaterThanOrEqual(9)
  for (const path of paths) expect(html, `reference page must mention ${path}`).toContain(path)
})
