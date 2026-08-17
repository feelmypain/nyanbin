import { constants } from 'node:fs'
import { open } from 'node:fs/promises'
import { basename, extname } from 'node:path'

import mime from 'mime'
import { type APIClient, type ReserveRequest, type ReserveResponse } from '../shared/api.js'
import {
  PROTOCOL_VERSION,
  buildNoteUrl,
  encodeBase64Url,
  encryptPayload,
  generateSecret,
  hashDeleteToken,
  validatePayload,
  validatePortableFilename,
  type PrivateFile,
  type PrivatePayload,
} from '../shared/protocol.js'

export type UploadOptions = ReserveRequest & {
  password?: string
  format?: PrivatePayload['format']
  files?: string[]
}

export type UploadResult = {
  id: string
  url: string
  deleteToken: string
  lifecycle: ReserveResponse['lifecycle']
}

export async function readPrivateFiles(paths: string[]): Promise<PrivateFile[]> {
  return Promise.all(
    paths.map(async (path) => {
      const name = basename(path)
      validatePortableFilename(name)
      if (!constants.O_NOFOLLOW) throw new Error('this platform cannot safely open attachments without following links')
      const handle = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW)
      try {
        const metadata = await handle.stat()
        if (!metadata.isFile()) throw new Error(`${path} is not a regular file`)
        const contents = await handle.readFile()
        return {
          name,
          type: mime.getType(extname(name)) ?? 'application/octet-stream',
          size: contents.byteLength,
          data: encodeBase64Url(contents),
        }
      } finally {
        await handle.close()
      }
    })
  )
}

export async function createPayload(text: string, options: Pick<UploadOptions, 'format' | 'files'> = {}): Promise<PrivatePayload> {
  const payload: PrivatePayload = {
    kind: 'text',
    format: options.format ?? 'plain',
    text,
    files: await readPrivateFiles(options.files ?? []),
  }
  validatePayload(payload)
  if (payload.text.length === 0 && payload.files.length === 0) throw new Error('a note must contain text or at least one file')
  return payload
}

export async function uploadPayload(payload: PrivatePayload, options: ReserveRequest & { password?: string }, api: APIClient): Promise<UploadResult> {
  validatePayload(payload)
  const reservation = await api.reserve({
    expiresIn: options.expiresIn,
    ...(options.maxReads === undefined ? {} : { maxReads: options.maxReads }),
  })
  const secret = generateSecret()
  const envelope = await encryptPayload(payload, {
    id: reservation.id,
    lifecycle: reservation.lifecycle,
    secret,
    ...(options.password === undefined ? {} : { password: options.password }),
  })
  const deleteTokenHash = await hashDeleteToken(reservation.deleteToken)
  await api.commit(reservation.id, {
    protocol: PROTOCOL_VERSION,
    envelope,
    lifecycle: reservation.lifecycle,
    deleteTokenHash,
    ...(options.password === undefined ? {} : { passwordProtected: true }),
  })
  return {
    id: reservation.id,
    url: buildNoteUrl(api.server, reservation.id, secret),
    deleteToken: reservation.deleteToken,
    lifecycle: reservation.lifecycle,
  }
}

export async function upload(input: string | string[], options: UploadOptions, api: APIClient): Promise<UploadResult> {
  const payload = Array.isArray(input)
    ? await createPayload('', { format: 'plain', files: input })
    : await createPayload(input, { format: options.format, files: options.files })
  return uploadPayload(payload, options, api)
}
