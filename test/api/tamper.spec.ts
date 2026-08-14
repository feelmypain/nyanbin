import { expect, test } from '@playwright/test'
import {
  decodeBase64Url,
  decryptPayload,
  encodeBase64Url,
  encryptPayload,
  generateSecret,
  type PrivatePayload,
} from '../../packages/cli/src/shared/protocol'

const id = '0123456789ABCDEFGHIJKLMNOPQRSTUV'
const lifecycle = { expiresAt: 2_000_000_000_000, maxReads: 2 }
const payload: PrivatePayload = {
  kind: 'text',
  format: 'plain',
  text: 'Authenticated ciphertext cannot be modified.',
  files: [],
}

test('authenticated envelope rejects ciphertext and lifecycle tampering', async () => {
  const secret = generateSecret()
  const envelope = await encryptPayload(payload, { id, lifecycle, secret })
  const bytes = decodeBase64Url(envelope)
  bytes[bytes.length - 1] ^= 1

  await expect(decryptPayload(encodeBase64Url(bytes), { id, secret })).rejects.toMatchObject({
    code: 'AUTHENTICATION_FAILED',
  })
  await expect(
    decryptPayload(envelope, { id, lifecycle: { ...lifecycle, maxReads: 3 }, secret }),
  ).rejects.toMatchObject({ code: 'INVALID_ENVELOPE' })
})
