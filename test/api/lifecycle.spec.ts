import { expect, test } from '@playwright/test'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { CLI, getLinkFromCLI, parseNoteLink, SERVER } from '../utils'

const exec = promisify(execFile)

async function valkeyTimeMs(): Promise<number> {
  const { stdout } = await exec(
    'docker',
    ['compose', '-f', 'docker-compose.dev.yaml', 'exec', '-T', 'valkey', 'valkey-cli', '--raw', 'TIME'],
    { cwd: process.cwd() },
  )
  const [seconds, microseconds] = stdout.trim().split(/\s+/).map(Number)
  if (!Number.isSafeInteger(seconds) || !Number.isSafeInteger(microseconds)) {
    throw new Error(`Invalid Valkey TIME response: ${stdout}`)
  }
  return seconds * 1000 + Math.floor(microseconds / 1000)
}

test('reservation expiry is minted from Valkey time and explicit zero removes the read cap', async () => {
  const before = await valkeyTimeMs()
  const response = await fetch(`${SERVER}/api/notes/reserve`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ expiresIn: 3600, maxReads: 0 }),
  })
  const after = await valkeyTimeMs()

  expect(response.status).toBe(201)
  const reservation = (await response.json()) as {
    lifecycle: { expiresAt: number; maxReads?: number }
  }
  expect(reservation.lifecycle.expiresAt).toBeGreaterThanOrEqual(before + 3_600_000)
  expect(reservation.lifecycle.expiresAt).toBeLessThanOrEqual(after + 3_600_000)
  expect(reservation.lifecycle).not.toHaveProperty('maxReads')
})

test('omitted maxReads uses the configured default', async () => {
  const response = await fetch(`${SERVER}/api/notes/reserve`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ expiresIn: 3600 }),
  })
  expect(response.status).toBe(201)
  await expect(response.json()).resolves.toMatchObject({ lifecycle: { maxReads: 1 } })
})

test('concurrent reveal consumes exactly the configured read cap', async () => {
  const created = await CLI('create', 'text', 'Atomic across simultaneous readers.', '--max-reads', '3', '--expires', '1h')
  const link = getLinkFromCLI(created.stdout)
  const { server, id } = parseNoteLink(link)

  const responses = await Promise.all(
    Array.from({ length: 16 }, () => fetch(`${server}/api/notes/${id}/reveal`, { method: 'POST' })),
  )
  expect(responses.filter((response) => response.status === 200)).toHaveLength(3)
  expect(responses.filter((response) => response.status === 404 || response.status === 410)).toHaveLength(13)
})
