import { expect, test } from '@playwright/test'
import { commitNote, OPS_SERVER, reserveNote, waitReady } from '../utils'

// Rate limiting is server-side and stateful; run once, on Chromium, strictly in order —
// every test below draws from the same per-client buckets on the ops instance.
test.skip(({ browserName }) => browserName !== 'chromium', 'API contract specs run once, on Chromium')
test.describe.configure({ mode: 'serial' })

// Mirrors the ops service overrides in docker-compose.dev.yaml.
const INFO_LIMIT = 5
const REVEAL_LIMIT = 4
const DELETE_LIMIT = 3
const BYTE_QUOTA = 131_072
const WINDOW_SECONDS = 60

function expectRetryAfter(response: Response, max: number): void {
  const retryAfter = response.headers.get('retry-after')
  expect(retryAfter).not.toBeNull()
  expect(retryAfter).toMatch(/^\d+$/)
  const seconds = Number(retryAfter)
  expect(seconds).toBeGreaterThanOrEqual(1)
  expect(seconds).toBeLessThanOrEqual(max)
}

async function expectRateLimited(response: Response, retryAfterMax: number): Promise<void> {
  expect(response.status).toBe(429)
  expectRetryAfter(response, retryAfterMax)
  const body = (await response.json()) as Record<string, unknown>
  expect(Object.keys(body).sort()).toEqual(['code', 'message'])
  expect(body.code).toBe('rate_limited')
}

test.beforeAll(async () => {
  await waitReady(OPS_SERVER)
})

test(`info bucket admits ${INFO_LIMIT} requests per window, then 429 with Retry-After`, async () => {
  const reservation = await reserveNote(OPS_SERVER, { maxReads: 0 })
  const { response: commit } = await commitNote(OPS_SERVER, reservation, { textBytes: 64 })
  expect([200, 201]).toContain(commit.status)

  for (let attempt = 1; attempt <= INFO_LIMIT; attempt += 1) {
    const response = await fetch(`${OPS_SERVER}/api/notes/${reservation.id}`)
    expect(response.status, `info request ${attempt} must still be admitted`).toBe(200)
  }
  const limited = await fetch(`${OPS_SERVER}/api/notes/${reservation.id}`)
  await expectRateLimited(limited, WINDOW_SECONDS)
})

test(`reveal bucket admits ${REVEAL_LIMIT} requests per window, then 429`, async () => {
  // Unlimited reads: only the rate limiter may refuse, never read exhaustion.
  const reservation = await reserveNote(OPS_SERVER, { maxReads: 0 })
  const { response: commit } = await commitNote(OPS_SERVER, reservation, { textBytes: 64 })
  expect([200, 201]).toContain(commit.status)

  for (let attempt = 1; attempt <= REVEAL_LIMIT; attempt += 1) {
    const response = await fetch(`${OPS_SERVER}/api/notes/${reservation.id}/reveal`, { method: 'POST' })
    expect(response.status, `reveal request ${attempt} must still be admitted`).toBe(200)
  }
  const limited = await fetch(`${OPS_SERVER}/api/notes/${reservation.id}/reveal`, { method: 'POST' })
  await expectRateLimited(limited, WINDOW_SECONDS)
})

test(`delete bucket admits ${DELETE_LIMIT} attempts per window, then 429`, async () => {
  const reservation = await reserveNote(OPS_SERVER, { maxReads: 0 })
  const { response: commit } = await commitNote(OPS_SERVER, reservation, { textBytes: 64 })
  expect([200, 201]).toContain(commit.status)

  // Wrong-but-well-formed capability: 43 base64url chars that hash to nothing we committed.
  const forgedToken = 'A'.repeat(43)
  expect(forgedToken).not.toBe(reservation.deleteToken)

  for (let attempt = 1; attempt <= DELETE_LIMIT; attempt += 1) {
    const response = await fetch(`${OPS_SERVER}/api/notes/${reservation.id}`, {
      method: 'DELETE',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ deleteToken: forgedToken }),
    })
    expect(response.status, `forged delete ${attempt} must be refused, not throttled`).toBe(403)
  }
  const limited = await fetch(`${OPS_SERVER}/api/notes/${reservation.id}`, {
    method: 'DELETE',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ deleteToken: forgedToken }),
  })
  await expectRateLimited(limited, WINDOW_SECONDS)
})

// Last on purpose: exhausting the hourly byte quota blocks every later commit on this
// instance until the hour boundary (which is also why local re-runs within the same hour
// see this trip earlier — the bucket is hourly by design).
test('hourly upload byte quota trips 429 with Retry-After up to the hour boundary', async () => {
  const bigNoteBytes = 64 * 1024
  let committedBytes = 0
  let limited: Response | undefined

  // Quota 131072 with ~64 KiB envelopes: the trip must come on the 2nd or 3rd commit.
  for (let attempt = 1; attempt <= 4; attempt += 1) {
    const reservation = await reserveNote(OPS_SERVER, { maxReads: 0 })
    const { response, envelopeBytes } = await commitNote(OPS_SERVER, reservation, {
      textBytes: bigNoteBytes,
    })
    if (response.status === 429) {
      limited = response
      break
    }
    expect([200, 201], `commit ${attempt} must succeed under the quota`).toContain(response.status)
    committedBytes += envelopeBytes
    expect(attempt, 'quota must trip by the third large commit').toBeLessThan(3 + 1)
  }

  expect(limited, 'byte quota must eventually refuse a commit').toBeDefined()
  expect(committedBytes).toBeLessThanOrEqual(BYTE_QUOTA)
  await expectRateLimited(limited as Response, 3600)
})
