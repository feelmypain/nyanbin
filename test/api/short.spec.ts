import { expect, test } from '@playwright/test'
import { createAPI, encryptPayload, generateSecret, hashDeleteToken, PROTOCOL_VERSION, type PrivatePayload } from '../../packages/cli/src/shared/shared'
import { SERVER } from '../utils'

const payload: PrivatePayload = {
  kind: 'text',
  format: 'plain',
  text: 'Short codes gate on the second factor.',
  files: [],
}

async function createNote(options: { password?: string } = {}): Promise<{ id: string; deleteToken: string }> {
  const api = createAPI({ server: SERVER })
  const reservation = await api.reserve({ expiresIn: 3600, maxReads: 5 })
  const secret = generateSecret()
  const envelope = await encryptPayload(payload, {
    id: reservation.id,
    lifecycle: reservation.lifecycle,
    secret,
    ...(options.password === undefined ? {} : { password: options.password }),
  })
  await api.commit(reservation.id, {
    protocol: PROTOCOL_VERSION,
    envelope,
    lifecycle: reservation.lifecycle,
    deleteTokenHash: await hashDeleteToken(reservation.deleteToken),
    ...(options.password === undefined ? {} : { passwordProtected: true }),
  })
  return { id: reservation.id, deleteToken: reservation.deleteToken }
}

test('short creation is refused for notes without a password', async () => {
  const { id, deleteToken } = await createNote()
  const response = await fetch(`${SERVER}/api/notes/${id}/short`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ deleteToken }),
  })
  expect(response.status).toBe(409)
  const body = (await response.json()) as { code: string }
  expect(body.code).toBe('short_link_requires_password')
})

test('short creation requires the real delete capability', async () => {
  const { id } = await createNote({ password: 'nyan' })
  const forged = 'A'.repeat(43)
  const response = await fetch(`${SERVER}/api/notes/${id}/short`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ deleteToken: forged }),
  })
  expect(response.status).toBe(403)
})

test('short codes resolve to the note id and are idempotent per note', async () => {
  const { id, deleteToken } = await createNote({ password: 'nyan' })
  const api = createAPI({ server: SERVER })
  const first = await api.createShort(id, deleteToken)
  const second = await api.createShort(id, deleteToken)
  expect(second.code).toBe(first.code)
  const resolved = await api.resolveShort(first.code)
  expect(resolved.id).toBe(id)
})

test('unknown short codes return 404', async () => {
  const response = await fetch(`${SERVER}/api/short/000000`)
  expect(response.status).toBe(404)
})
