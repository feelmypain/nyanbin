import { expect, test } from '@playwright/test'
import {
  commitNote,
  PRESSURE_SERVER,
  reserveNote,
  valkeyCli,
  waitReady,
  type ApiReservation,
} from '../utils'

// Storage pressure and operator switches mutate instance-wide state; run once, on
// Chromium, strictly in order. The pressure instance is isolated by its Redis prefix.
test.skip(({ browserName }) => browserName !== 'chromium', 'API contract specs run once, on Chromium')
test.describe.configure({ mode: 'serial' })

// Mirrors the pressure service overrides in docker-compose.dev.yaml.
const STORAGE_BUDGET = 100_000
const LARGE_COMMIT_THRESHOLD = 65_536
const SOFT_PRESSURE_BYTES = Math.ceil(STORAGE_BUDGET * 0.9)
const HARD_PRESSURE_BYTES = Math.ceil(STORAGE_BUDGET * 0.98)

const committed: Array<Pick<ApiReservation, 'id' | 'deleteToken'>> = []
let occupiedBytes = 0

async function expectErrorBody(response: Response, status: number, code: string): Promise<void> {
  expect(response.status).toBe(status)
  const body = (await response.json()) as Record<string, unknown>
  expect(Object.keys(body).sort()).toEqual(['code', 'message'])
  expect(body.code).toBe(code)
}

// Commits a note expected to succeed and tracks it for meter accounting and cleanup.
async function commitTracked(textBytes: number): Promise<void> {
  const reservation = await reserveNote(PRESSURE_SERVER, { maxReads: 0 })
  const { response, envelopeBytes } = await commitNote(PRESSURE_SERVER, reservation, { textBytes })
  expect([200, 201], `commit of ~${textBytes} bytes must succeed below pressure`).toContain(
    response.status,
  )
  committed.push({ id: reservation.id, deleteToken: reservation.deleteToken })
  occupiedBytes += envelopeBytes
}

test.beforeAll(async () => {
  await waitReady(PRESSURE_SERVER)
})

// Deleting the committed notes decrements the storage meter, so local re-runs start from
// a drained budget instead of a poisoned one. Switch keys are likewise removed.
test.afterAll(async () => {
  await valkeyCli('DEL', 'nyanbin:pressure:switch:writes_off', 'nyanbin:pressure:switch:short_off')
  for (const note of committed) {
    await fetch(`${PRESSURE_SERVER}/api/notes/${note.id}`, {
      method: 'DELETE',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ deleteToken: note.deleteToken }),
    }).catch(() => {})
  }
})

// Approaches a meter target from below without overshooting the next threshold: each
// chunk is sized to the remaining gap (envelope framing adds ~150 bytes over the text).
async function fillTo(targetBytes: number): Promise<void> {
  while (occupiedBytes < targetBytes) {
    const gap = targetBytes - occupiedBytes
    await commitTracked(Math.min(40 * 1024, Math.max(64, gap)))
  }
}

test('storage pressure brownout: large commits go first, then all commits', async () => {
  // Baseline: an empty meter accepts anything.
  await commitTracked(64)

  // Fill just past the soft threshold (chunks stay far below the large-commit line).
  await fillTo(SOFT_PRESSURE_BYTES)
  expect(occupiedBytes).toBeGreaterThanOrEqual(SOFT_PRESSURE_BYTES)
  expect(occupiedBytes).toBeLessThan(HARD_PRESSURE_BYTES)

  // ≥90%: large commits (decoded envelope > 65536 bytes) are refused with 507 ...
  const largeReservation = await reserveNote(PRESSURE_SERVER, { maxReads: 0 })
  const { response: largeCommit, envelopeBytes: largeBytes } = await commitNote(
    PRESSURE_SERVER,
    largeReservation,
    { textBytes: LARGE_COMMIT_THRESHOLD + 4096 },
  )
  expect(largeBytes).toBeGreaterThan(LARGE_COMMIT_THRESHOLD)
  await expectErrorBody(largeCommit, 507, 'storage_pressure')

  // ... while small commits still land.
  await commitTracked(1024)

  // Keep filling just past ≥98%: now every commit is refused, no matter how small.
  await fillTo(HARD_PRESSURE_BYTES)
  const smallReservation = await reserveNote(PRESSURE_SERVER, { maxReads: 0 })
  const { response: refusedCommit } = await commitNote(PRESSURE_SERVER, smallReservation, {
    textBytes: 64,
  })
  await expectErrorBody(refusedCommit, 507, 'storage_pressure')
})

test('writes_off switch disables reserve and recovers when cleared', async () => {
  await valkeyCli('SET', 'nyanbin:pressure:switch:writes_off', '1')
  try {
    const refused = await fetch(`${PRESSURE_SERVER}/api/notes/reserve`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ expiresIn: 3600 }),
    })
    await expectErrorBody(refused, 503, 'writes_disabled')
  } finally {
    await valkeyCli('DEL', 'nyanbin:pressure:switch:writes_off')
  }

  // Cleared: reserve works again (the storage meter is still hot, so stop at reserve).
  const recovered = await reserveNote(PRESSURE_SERVER, { maxReads: 0 })
  expect(recovered.id).toMatch(/^[0-9A-Za-z]{32}$/)
})

test('short_off switch disables short create and resolve and recovers when cleared', async () => {
  // Short codes require a password-protected note; delete a tracked note first so the
  // hot storage meter accepts this one small commit.
  const sacrifice = committed.pop()
  expect(sacrifice, 'pressure test must have committed notes to reclaim').toBeDefined()
  const freed = await fetch(`${PRESSURE_SERVER}/api/notes/${(sacrifice as ApiReservation).id}`, {
    method: 'DELETE',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ deleteToken: (sacrifice as ApiReservation).deleteToken }),
  })
  expect(freed.status).toBe(204)

  const reservation = await reserveNote(PRESSURE_SERVER, { maxReads: 0 })
  const { response: commit } = await commitNote(PRESSURE_SERVER, reservation, {
    textBytes: 64,
    password: 'nyan',
  })
  expect([200, 201]).toContain(commit.status)
  committed.push({ id: reservation.id, deleteToken: reservation.deleteToken })

  const short = await fetch(`${PRESSURE_SERVER}/api/notes/${reservation.id}/short`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ deleteToken: reservation.deleteToken }),
  })
  expect([200, 201]).toContain(short.status)
  const { code } = (await short.json()) as { code: string }

  await valkeyCli('SET', 'nyanbin:pressure:switch:short_off', '1')
  try {
    const refusedCreate = await fetch(`${PRESSURE_SERVER}/api/notes/${reservation.id}/short`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ deleteToken: reservation.deleteToken }),
    })
    await expectErrorBody(refusedCreate, 503, 'short_disabled')

    const refusedResolve = await fetch(`${PRESSURE_SERVER}/api/short/${code}`)
    await expectErrorBody(refusedResolve, 503, 'short_disabled')
  } finally {
    await valkeyCli('DEL', 'nyanbin:pressure:switch:short_off')
  }

  // Cleared: resolution works again and still points at the note.
  const resolved = await fetch(`${PRESSURE_SERVER}/api/short/${code}`)
  expect(resolved.status).toBe(200)
  await expect(resolved.json()).resolves.toEqual({ id: reservation.id })
})
