import { expect, type Page } from '@playwright/test'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { getFileChecksum } from './files'
import {
  decodeBase64Url,
  encryptPayload,
  generateSecret,
  hashDeleteToken,
  PROTOCOL_VERSION,
  type PrivatePayload,
} from '../packages/cli/src/shared/shared'

const exec = promisify(execFile)
export const SERVER = process.env.NYANBIN_E2E_URL ?? 'http://127.0.0.1:3000'
export const OPS_SERVER = 'http://127.0.0.1:3001'
export const PRESSURE_SERVER = 'http://127.0.0.1:3002'

// The Playwright webServer only waits for the primary app on :3000; specs targeting the
// auxiliary compose services poll readiness themselves before exercising the API.
export async function waitReady(server: string): Promise<void> {
  await expect
    .poll(
      async () => {
        try {
          const response = await fetch(`${server}/api/ready`)
          return response.status
        } catch {
          return 0
        }
      },
      { timeout: 60_000, message: `waiting for ${server}/api/ready` },
    )
    .toBe(200)
}

export type ApiReservation = {
  id: string
  deleteToken: string
  lifecycle: { expiresAt: number; maxReads?: number }
}

export async function reserveNote(
  server: string,
  options: { expiresIn?: number; maxReads?: number } = {},
): Promise<ApiReservation> {
  const response = await fetch(`${server}/api/notes/reserve`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ expiresIn: options.expiresIn ?? 3600, maxReads: options.maxReads ?? 5 }),
  })
  expect(response.status).toBe(201)
  return (await response.json()) as ApiReservation
}

// Commits a locally encrypted envelope for a reservation and returns the raw response so
// callers can assert quota/pressure statuses. `textBytes` pads the plaintext, so the decoded
// envelope lands within a few hundred bytes above it; the exact decoded size is returned.
export async function commitNote(
  server: string,
  reservation: ApiReservation,
  options: { textBytes?: number; password?: string } = {},
): Promise<{ response: Response; envelopeBytes: number; requestBody: string }> {
  const payload: PrivatePayload = {
    kind: 'text',
    format: 'plain',
    text: 'n'.repeat(options.textBytes ?? 64),
    files: [],
  }
  const envelope = await encryptPayload(payload, {
    id: reservation.id,
    lifecycle: reservation.lifecycle,
    secret: generateSecret(),
    ...(options.password === undefined ? {} : { password: options.password }),
  })
  const requestBody = JSON.stringify({
    protocol: PROTOCOL_VERSION,
    envelope,
    lifecycle: reservation.lifecycle,
    deleteTokenHash: await hashDeleteToken(reservation.deleteToken),
    ...(options.password === undefined ? {} : { passwordProtected: true }),
  })
  const response = await fetch(`${server}/api/notes/${reservation.id}`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: requestBody,
  })
  return { response, envelopeBytes: decodeBase64Url(envelope).length, requestBody }
}

export async function valkeyCli(...args: string[]): Promise<string> {
  const { stdout } = await exec(
    'docker',
    ['compose', '-f', 'docker-compose.dev.yaml', 'exec', '-T', 'valkey', 'valkey-cli', '--raw', ...args],
    { cwd: process.cwd() },
  )
  return stdout.trim()
}

export type CreateOptions = {
  text?: string
  files?: string[]
  format?: 'plain' | 'source' | 'markdown'
  expiresIn?: 3600 | 21600 | 86400 | 604800
  maxReads?: number
  password?: string
}

export async function createNoteSuccessfully(page: Page, options: CreateOptions): Promise<string> {
  await page.goto('/')
  await expect(page.getByTestId('create-form')).toBeVisible()

  if (options.format) await page.getByTestId(`format-${options.format}`).click()
  if (options.text !== undefined) await page.getByTestId('text-field').fill(options.text)
  if (options.files?.length) await page.getByTestId('file-upload').setInputFiles(options.files)
  if (options.expiresIn) await page.getByTestId('field-expiry').selectOption(String(options.expiresIn))
  if (options.maxReads !== undefined) {
    const toggle = page.getByTestId('read-cap-toggle')
    if (!(await toggle.isChecked())) await toggle.check()
    await page.getByTestId('field-reads').fill(String(options.maxReads))
  }
  if (options.password) {
    await page.getByTestId('password-toggle').check()
    await page.getByTestId('password').fill(options.password)
  }

  await page.getByTestId('create-button').click()
  await expect(page.getByTestId('create-result')).toBeVisible()
  const link = await page.getByTestId('share-link').inputValue()
  if (options.password) {
    // Password notes are keyed by the password alone; their links stay bare.
    expect(link).toMatch(/^https?:\/\/[^\s]+\/note\/[A-Za-z0-9]{32}$/)
  } else {
    expect(link).toMatch(/^https?:\/\/[^\s]+\/note\/[A-Za-z0-9]{32}#[A-Za-z0-9_-]{43}$/)
  }
  return link
}

export async function reveal(page: Page, link: string, password?: string): Promise<void> {
  await page.goto('about:blank')
  await page.goto(link)
  await expect(page.getByTestId('reveal-gate')).toBeVisible()
  if (password) await page.getByTestId('show-note-password').fill(password)
  await page.getByTestId('show-note-button').click()
  await expect(page.getByTestId('result')).toBeVisible()
}

export async function checkLinkForText(
  page: Page,
  options: { link: string; text: string; password?: string },
): Promise<void> {
  await reveal(page, options.link, options.password)
  await expect(page.getByTestId('result')).toContainText(options.text)
}

export async function checkLinkForDownload(
  page: Page,
  options: { link: string; index?: number; checksum: string; password?: string },
): Promise<void> {
  await reveal(page, options.link, options.password)
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    page.getByTestId(`download-file-${options.index ?? 0}`).click(),
  ])
  const path = await download.path()
  if (!path) throw new Error('Download failed')
  expect(await getFileChecksum(path)).toBe(options.checksum)
}

export function parseNoteLink(link: string): { server: string; id: string; secret?: string } {
  const url = new URL(link)
  const match = /^\/note\/([A-Za-z0-9]{32})$/.exec(url.pathname)
  if (!match) throw new Error(`Invalid Nyanbin note link: ${link}`)
  return { server: url.origin, id: match[1], ...(url.hash ? { secret: url.hash.slice(1) } : {}) }
}

export async function expectNoteMissing(link: string): Promise<void> {
  const { server, id } = parseNoteLink(link)
  const response = await fetch(`${server}/api/notes/${id}`)
  expect([404, 410]).toContain(response.status)
}

export async function CLI(...args: string[]) {
  return exec('./packages/cli/dist/cli.cjs', args, {
    env: { ...process.env, NYANBIN_SERVER: SERVER },
  })
}

export function getLinkFromCLI(output: string): string {
  const match = /^Note:\s+(https?:\/\/\S+)$/m.exec(output)
  if (!match) throw new Error(`No labelled note link found in CLI output: ${output}`)
  return match[1]
}

export function getDeleteTokenFromCLI(output: string): string {
  const match = /^Delete token:\s+([A-Za-z0-9_-]{43})$/m.exec(output)
  if (!match) throw new Error(`No labelled delete token found in CLI output: ${output}`)
  return match[1]
}

export async function CLIAt(cwd: string, ...args: string[]) {
  return exec(`${process.cwd()}/packages/cli/dist/cli.cjs`, args, {
    cwd,
    env: { ...process.env, NYANBIN_SERVER: SERVER },
  })
}
