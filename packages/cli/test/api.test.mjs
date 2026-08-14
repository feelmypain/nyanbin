import assert from 'node:assert/strict'
import test from 'node:test'

import { API, createAPI } from '../dist/shared/api.js'
import { encodeBase64Url, encryptPayload, hashDeleteToken } from '../dist/shared/protocol.js'

const id = '0123456789abcdefghijklmnopqrstuv'
const deleteToken = encodeBase64Url(new Uint8Array(32).fill(7))
const lifecycle = { expiresAt: 1_900_000_000_000, maxReads: 2 }

function json(data, status = 200) {
  return new Response(JSON.stringify(data), { status, headers: { 'Content-Type': 'application/json' } })
}

test('API uses reserve, commit, passive info, consuming reveal, and body delete paths', async () => {
  const secret = new Uint8Array(32).fill(3)
  const envelope = await encryptPayload({ kind: 'text', format: 'plain', text: 'nya', files: [] }, { id, lifecycle, secret })
  const calls = []
  const api = createAPI({
    server: 'https://notes.example',
    async fetch(url, init) {
      calls.push({ url, method: init.method, body: init.body === undefined ? undefined : JSON.parse(init.body) })
      if (url.endsWith('/reserve')) return json({ id, deleteToken, lifecycle })
      if (init.method === 'PUT') return json({ id })
      if (init.method === 'GET') return json({ protocol: 1, lifecycle: { ...lifecycle, remainingReads: 2 } })
      if (url.endsWith('/reveal')) return json({ protocol: 1, envelope })
      return new Response(null, { status: 204 })
    },
  })

  const reserved = await api.reserve({ expiresIn: 3600, maxReads: 2 })
  assert.deepEqual(reserved.lifecycle, lifecycle)
  await api.commit(id, { protocol: 1, envelope, lifecycle, deleteTokenHash: await hashDeleteToken(deleteToken) })
  await api.info(id)
  await api.reveal(id)
  await api.delete(id, deleteToken)

  assert.deepEqual(calls.map(({ url, method }) => [new URL(url).pathname, method]), [
    ['/api/notes/reserve', 'POST'],
    [`/api/notes/${id}`, 'PUT'],
    [`/api/notes/${id}`, 'GET'],
    [`/api/notes/${id}/reveal`, 'POST'],
    [`/api/notes/${id}`, 'DELETE'],
  ])
  assert.deepEqual(calls[0].body, { expiresIn: 3600, maxReads: 2 })
  assert.deepEqual(calls[4].body, { deleteToken })
})

test('typed API errors preserve server code and reject malformed responses', async () => {
  const gone = createAPI({
    server: 'https://notes.example',
    fetch: async () => json({ code: 'NOTE_GONE', message: 'note is gone' }, 404),
  })
  await assert.rejects(gone.info(id), (error) => error.code === 'NOTE_GONE' && error.status === 404)

  const malformed = createAPI({ server: 'https://notes.example', fetch: async () => json({ protocol: 1, envelope: 'not base64url!' }) })
  await assert.rejects(malformed.reveal(id), (error) => error.code === 'INVALID_BASE64URL')
})

test('reserve accepts zero as the uncapped request sentinel but lifecycle maxReads stays positive', async () => {
  let requestBody
  const uncapped = createAPI({
    server: 'https://notes.example',
    fetch: async (_url, init) => {
      requestBody = JSON.parse(init.body)
      return json({ id, deleteToken, lifecycle: { expiresAt: lifecycle.expiresAt } })
    },
  })
  const reservation = await uncapped.reserve({ expiresIn: 3600, maxReads: 0 })
  assert.deepEqual(requestBody, { expiresIn: 3600, maxReads: 0 })
  assert.deepEqual(reservation.lifecycle, { expiresAt: lifecycle.expiresAt })

  const invalidLifecycle = createAPI({
    server: 'https://notes.example',
    fetch: async () => json({ id, deleteToken, lifecycle: { expiresAt: lifecycle.expiresAt, maxReads: 0 } }),
  })
  await assert.rejects(invalidLifecycle.reserve({ expiresIn: 3600, maxReads: 0 }), (error) => error.code === 'INVALID_LIFECYCLE')
})

test('clients are immutable and concurrent origins stay isolated across awaits', async () => {
  const calls = []
  let releaseFirst
  const firstWaiting = new Promise((resolve) => { releaseFirst = resolve })
  const makeFetch = (origin, wait) => async (url, init) => {
    calls.push([origin, url, init.method])
    if (wait && url.endsWith('/reserve')) await firstWaiting
    return json({ id, deleteToken, lifecycle: { expiresAt: lifecycle.expiresAt } })
  }
  const first = createAPI({ server: 'https://first.example/', fetch: makeFetch('first', true) })
  const second = createAPI({ server: 'https://second.example', fetch: makeFetch('second', false) })

  assert.equal(Object.isFrozen(API), true)
  assert.equal('setOptions' in API, false)
  assert.equal('getOptions' in API, false)
  assert.equal(Object.isFrozen(first), true)
  assert.equal(first.server, 'https://first.example')
  const firstOperation = first.reserve({ expiresIn: 3600, maxReads: 0 })
  await second.reserve({ expiresIn: 3600, maxReads: 0 })
  releaseFirst()
  await firstOperation

  assert.deepEqual(calls.map(([label, url]) => [label, new URL(url).origin]), [
    ['first', 'https://first.example'],
    ['second', 'https://second.example'],
  ])
})
