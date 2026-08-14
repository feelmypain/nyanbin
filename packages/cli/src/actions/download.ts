import inquirer from 'inquirer'
import { constants } from 'node:fs'
import { open } from 'node:fs/promises'
import { basename, extname, resolve } from 'node:path'
import type { Writable } from 'node:stream'
import pretty from 'pretty-bytes'

import { createAPI } from '../shared/api.js'
import { decodeBase64Url, decryptPayload, parseNoteLink, validatePayload, type PrivatePayload } from '../shared/protocol.js'

export type DownloadOptions = {
  all?: boolean
  password?: string
  raw?: boolean
}

const OUTPUT_FLAGS = constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW
const TERMINAL_CONTROL = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f\u061c\u200e\u200f\u2028-\u202e\u2066-\u2069]/gu
const TERMINAL_LINE_CONTROL = /[\u0000-\u001f\u007f-\u009f\u061c\u200e\u200f\u2028-\u202e\u2066-\u2069]/gu

export function escapeTerminal(value: string): string {
  return value.replace(TERMINAL_CONTROL, (character) => `\\u{${character.codePointAt(0)!.toString(16).padStart(4, '0')}}`)
}
export function escapeTerminalLine(value: string): string {
  return value.replace(TERMINAL_LINE_CONTROL, (character) => `\\u{${character.codePointAt(0)!.toString(16).padStart(4, '0')}}`)
}

function utf8Prefix(value: string, maximumBytes: number): string {
  let result = ''
  let bytes = 0
  for (const character of value) {
    const characterBytes = Buffer.byteLength(character)
    if (bytes + characterBytes > maximumBytes) break
    result += character
    bytes += characterBytes
  }
  return result
}

function fitCollisionName(name: string, index: number): string {
  if (index === 0) return name
  const extension = extname(name)
  const suffix = ` (${index})`
  if (Buffer.byteLength(suffix + extension) <= 255) {
    const stem = name.slice(0, extension.length === 0 ? undefined : -extension.length)
    return `${utf8Prefix(stem, 255 - Buffer.byteLength(suffix + extension))}${suffix}${extension}`
  }
  return `${utf8Prefix(name, 255 - Buffer.byteLength(suffix))}${suffix}`
}

async function createOutputFile(directory: string, name: string, data: Uint8Array): Promise<string> {
  for (let index = 0; index < 10_000; index++) {
    const path = resolve(directory, fitCollisionName(name, index))
    try {
      const handle = await open(path, OUTPUT_FLAGS, 0o600)
      try {
        await handle.writeFile(data)
      } finally {
        await handle.close()
      }
      return path
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'EEXIST' || (error as NodeJS.ErrnoException).code === 'ELOOP') continue
      throw error
    }
  }
  throw new Error(`could not choose an unused output name for ${escapeTerminalLine(name)}`)
}

export function writeNoteText(text: string, raw: boolean, output: Writable = process.stdout): void {
  output.write(raw ? text : `${escapeTerminal(text)}\n`)
}

export async function saveFiles(payload: PrivatePayload, all: boolean, directory: string = process.cwd()): Promise<string[]> {
  validatePayload(payload)
  if (payload.files.length === 0) return []
  let selected = payload.files
  if (!all) {
    const response = await inquirer.prompt<{ names: string[] }>([
      {
        type: 'checkbox',
        message: 'Which files should be saved?',
        name: 'names',
        choices: payload.files.map((file) => ({
          value: file.name,
          name: `${escapeTerminalLine(file.name)} - ${escapeTerminalLine(file.type)} - ${pretty(file.size, { binary: true })}`,
          checked: true,
        })),
      },
    ])
    selected = payload.files.filter((file) => response.names.includes(file.name))
  }
  if (selected.length === 0) throw new Error('no files selected')
  return Promise.all(
    selected.map(async (file) =>
      createOutputFile(directory, file.name, decodeBase64Url(file.data, { length: file.size, label: `data for ${escapeTerminalLine(file.name)}` }))
    )
  )
}

export async function download(input: URL | string, allOrOptions: boolean | DownloadOptions = false, suggestedPassword?: string) {
  const options: DownloadOptions =
    typeof allOrOptions === 'boolean' ? { all: allOrOptions, password: suggestedPassword } : allOrOptions
  const link = parseNoteLink(input.toString())
  const api = createAPI({ server: link.server })

  // Info is passive and catches malformed links before consuming a read. Reveal consumes atomically before decryption.
  const noteInfo = await api.info(link.id)
  const revealed = await api.reveal(link.id)
  const payload = await decryptPayload(revealed.envelope, {
    id: link.id,
    lifecycle: { expiresAt: noteInfo.lifecycle.expiresAt, ...(noteInfo.lifecycle.maxReads === undefined ? {} : { maxReads: noteInfo.lifecycle.maxReads }) },
    secret: link.secret,
    ...(options.password === undefined ? {} : { password: options.password }),
  })

  if (payload.text.length > 0) writeNoteText(payload.text, options.raw ?? false)
  const saved = await saveFiles(payload, options.all ?? false)
  for (const path of saved) console.log(`Saved: ${escapeTerminalLine(basename(path))}`)
  return payload
}
