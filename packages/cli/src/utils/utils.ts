import { exit as exitNode } from 'node:process'
import { API, type APIClient, type ReserveRequest, type Status } from '../shared/api.js'
import { NyanbinError } from '../shared/protocol.js'

export function exit(message: string): never {
  console.error(message)
  exitNode(1)
}

export function errorMessage(error: unknown): string {
  if (error instanceof NyanbinError || error instanceof Error) return error.message
  return 'unknown error'
}

export function resolveLifecycle(
  options: { expiresIn?: number; maxReads?: number },
  status: Status
): ReserveRequest {
  const expiresIn = options.expiresIn ?? status.defaults.expiresIn
  const maxReads = options.maxReads ?? status.defaults.maxReads
  if (expiresIn > status.limits.maxExpiresIn) {
    throw new Error(`expiry exceeds the server maximum of ${status.limits.maxExpiresIn} seconds`)
  }
  if (maxReads !== undefined && maxReads > status.limits.maxReads) {
    throw new Error(`read limit exceeds the server maximum of ${status.limits.maxReads}`)
  }
  return { expiresIn, ...(maxReads === undefined ? {} : { maxReads }) }
}

export async function checkConstraints(options: { expiresIn?: number; maxReads?: number }, api: APIClient = API): Promise<ReserveRequest> {
  return resolveLifecycle(options, await api.status())
}

