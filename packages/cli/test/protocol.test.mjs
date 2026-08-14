import assert from 'node:assert/strict'
import test from 'node:test'

import {
  NyanbinError,
  canonicalAadString,
  decodeBase64Url,
  decryptPayload,
  encodeBase64Url,
  encryptPayload,
  parseEnvelope,
} from '../dist/shared/protocol.js'

const id = '0123456789abcdefghijklmnopqrstuv'
const lifecycle = { expiresAt: 1_900_000_000_000, maxReads: 3 }
const secret = Uint8Array.from({ length: 32 }, (_, index) => index)
const payload = {
  kind: 'text',
  format: 'markdown',
  text: 'Nyan 🐈\nこんにちは',
  files: [
    {
      name: 'blue cat.txt',
      type: 'text/plain',
      size: 5,
      data: encodeBase64Url(new TextEncoder().encode('nya!\n')),
    },
  ],
}

function errorCode(code) {
  return (error) => error instanceof NyanbinError && error.code === code
}

test('canonical AAD is deterministic and binds the complete immutable header', () => {
  assert.equal(
    canonicalAadString(id, lifecycle),
    '{"protocol":1,"id":"0123456789abcdefghijklmnopqrstuv","expiresAt":1900000000000,"maxReads":3}'
  )
  assert.equal(
    canonicalAadString(id, { expiresAt: lifecycle.expiresAt }),
    '{"protocol":1,"id":"0123456789abcdefghijklmnopqrstuv","expiresAt":1900000000000,"maxReads":null}'
  )
})

test('base64url decoder rejects padding, aliases, and non-zero trailing bits', () => {
  assert.deepEqual(decodeBase64Url('AA'), new Uint8Array([0]))
  assert.throws(() => decodeBase64Url('AA=='), errorCode('INVALID_BASE64URL'))
  assert.throws(() => decodeBase64Url('AB'), errorCode('INVALID_BASE64URL'))
  assert.throws(() => decodeBase64Url('A'), errorCode('INVALID_BASE64URL'))
})

test('fixed random bytes produce a deterministic interoperable envelope', async () => {
  const original = globalThis.crypto.getRandomValues.bind(globalThis.crypto)
  let next = 0
  Object.defineProperty(globalThis.crypto, 'getRandomValues', {
    configurable: true,
    value(array) {
      for (let index = 0; index < array.length; index++) array[index] = next++ & 0xff
      return array
    },
  })
  try {
    next = 0
    const first = await encryptPayload(payload, { id, lifecycle, secret, password: 'correct horse' })
    next = 0
    const second = await encryptPayload(payload, { id, lifecycle, secret, password: 'correct horse' })
    assert.equal(first, second)
    assert.equal(
      first,
      'ATAxMjM0NTY3ODlhYmNkZWZnaGlqa2xtbm9wcXJzdHV2AAABumDTOAAAAAADAAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaG59CLReEDYJzNc2hlzdB_0V3gCIaZStBCOHhDeE8UFJMF0b9M7D25dhEymWTxJV4-Y7BfzfguwOlQL8-AWOuHfmyQTHJccOSNgByLubWlKjuV1LeT7Il5YM2km9q7iEzy1viKqKX0kWtAVdly4YWkmLjHz9xV5PL3_Eoi-7gfOsq2y8B0wdTkkPGLnqROUty3mBjuupKOi8JliC9ASJg-4d2vjyj20-8'
    )
    assert.deepEqual(parseEnvelope(first).lifecycle, lifecycle)
    assert.deepEqual(await decryptPayload(first, { id, lifecycle, secret, password: 'correct horse' }), payload)
  } finally {
    Object.defineProperty(globalThis.crypto, 'getRandomValues', { configurable: true, value: original })
  }
})

test('wrong password and ciphertext tampering fail authentication', async () => {
  const envelope = await encryptPayload(payload, { id, lifecycle, secret, password: 'right' })
  await assert.rejects(
    decryptPayload(envelope, { id, lifecycle, secret, password: 'wrong' }),
    errorCode('AUTHENTICATION_FAILED')
  )
  const bytes = decodeBase64Url(envelope)
  bytes[bytes.length - 1] ^= 1
  await assert.rejects(
    decryptPayload(encodeBase64Url(bytes), { id, lifecycle, secret, password: 'right' }),
    errorCode('AUTHENTICATION_FAILED')
  )
})

test('header tampering and malformed links fail before decryption', async () => {
  const envelope = await encryptPayload(payload, { id, lifecycle, secret })
  const bytes = decodeBase64Url(envelope)
  bytes[1] = 'Z'.charCodeAt(0)
  await assert.rejects(
    decryptPayload(encodeBase64Url(bytes), { id, lifecycle, secret }),
    errorCode('INVALID_ENVELOPE')
  )
})
