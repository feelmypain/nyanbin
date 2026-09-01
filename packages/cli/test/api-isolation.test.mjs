import assert from 'node:assert/strict'
import test from 'node:test'

import { deleteNoteByLink, download, uploadPayload } from '../dist/index.js'
import { createAPI } from '../dist/shared/api.js'
import { buildNoteUrl, encodeBase64Url, encryptPayload } from '../dist/shared/protocol.js'

const firstId = '0123456789abcdefghijklmnopqrstuv'
const secondId = '1123456789abcdefghijklmnopqrstuv'
const deleteToken = encodeBase64Url(new Uint8Array(32).fill(7))
const lifecycle = { expiresAt: 1_900_000_000_000 }
const payload = { kind: 'text', format: 'plain', text: 'nya', files: [] }

function json(data, status = 200) {
  return new Response(JSON.stringify(data), { status, headers: { 'Content-Type': 'application/json' } })
}

function deferred() {
  let resolve
  const promise = new Promise((done) => { resolve = done })
  return { promise, resolve }
}

test('concurrent uploads keep reserve, commit, and generated links on their captured clients', async () => {
  const firstReserveStarted = deferred()
  const releaseFirstReserve = deferred()
  const calls = []
  const client = (origin, id, pause) => createAPI({
    server: origin,
    async fetch(url, init) {
      calls.push([origin, new URL(url).origin, init.method])
      if (url.endsWith('/reserve')) {
        if (pause) {
          firstReserveStarted.resolve()
          await releaseFirstReserve.promise
        }
        return json({ id, deleteToken, lifecycle })
      }
      return json({ id })
    },
  })
  const firstClient = client('https://first.example', firstId, true)
  const secondClient = client('https://second.example', secondId, false)

  const firstUpload = uploadPayload(payload, { expiresIn: 3600 }, firstClient)
  await firstReserveStarted.promise
  const secondUpload = await uploadPayload(payload, { expiresIn: 3600 }, secondClient)
  releaseFirstReserve.resolve()
  const firstResult = await firstUpload

  assert.equal(new URL(firstResult.url).origin, 'https://first.example')
  assert.equal(new URL(secondUpload.url).origin, 'https://second.example')
  assert.deepEqual(calls.map(([clientOrigin, requestOrigin]) => [clientOrigin, requestOrigin]), [
    ['https://first.example', 'https://first.example'],
    ['https://second.example', 'https://second.example'],
    ['https://second.example', 'https://second.example'],
    ['https://first.example', 'https://first.example'],
  ])
})

test('concurrent link operations keep info, reveal, and delete on each link origin', async () => {
  const firstInfoStarted = deferred()
  const releaseFirstInfo = deferred()
  const calls = []
  const firstSecret = new Uint8Array(32).fill(1)
  const secondSecret = new Uint8Array(32).fill(2)
  const envelopes = new Map([
    [firstId, await encryptPayload({ ...payload, text: '' }, { id: firstId, lifecycle, secret: firstSecret })],
    [secondId, await encryptPayload({ ...payload, text: '' }, { id: secondId, lifecycle, secret: secondSecret })],
  ])
  const originalFetch = globalThis.fetch
  globalThis.fetch = async (url, init) => {
    const parsed = new URL(url)
    const id = parsed.pathname.split('/')[3]
    calls.push([parsed.origin, parsed.pathname, init.method])
    if (parsed.origin === 'https://first.example' && init.method === 'GET') {
      firstInfoStarted.resolve()
      await releaseFirstInfo.promise
    }
    if (init.method === 'GET') return json({ protocol: 1, lifecycle, passwordProtected: false })
    if (parsed.pathname.endsWith('/reveal')) return json({ protocol: 1, envelope: envelopes.get(id) })
    return new Response(null, { status: 204 })
  }

  try {
    const firstLink = buildNoteUrl('https://first.example', firstId, firstSecret)
    const secondLink = buildNoteUrl('https://second.example', secondId, secondSecret)
    const firstDownload = download(firstLink)
    await firstInfoStarted.promise
    await download(secondLink)
    releaseFirstInfo.resolve()
    await firstDownload
    await Promise.all([
      deleteNoteByLink(firstLink, deleteToken),
      deleteNoteByLink(secondLink, deleteToken),
    ])
  } finally {
    globalThis.fetch = originalFetch
  }

  assert.deepEqual(calls.map(([origin, path, method]) => [origin, path.endsWith('/reveal') ? 'reveal' : method]), [
    ['https://first.example', 'GET'],
    ['https://second.example', 'GET'],
    ['https://second.example', 'reveal'],
    ['https://first.example', 'reveal'],
    ['https://first.example', 'DELETE'],
    ['https://second.example', 'DELETE'],
  ])
})
