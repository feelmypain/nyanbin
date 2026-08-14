import { InvalidArgumentError, InvalidOptionArgumentError } from '@commander-js/extra-typings'
import { resolve } from 'node:path'

export function parseFile(value: string, before: string[] = []): string[] {
  return [...before, resolve(value)]
}

export function parseURL(value: string): URL {
  try {
    const url = new URL(value)
    if (url.protocol !== 'http:' && url.protocol !== 'https:') throw new Error('unsupported protocol')
    return url
  } catch {
    throw new InvalidArgumentError('must be an absolute HTTP(S) URL')
  }
}

export function parsePositiveInteger(value: string): number {
  if (!/^[1-9]\d*$/.test(value)) throw new InvalidOptionArgumentError('must be a positive integer')
  const number = Number(value)
  if (!Number.isSafeInteger(number)) throw new InvalidOptionArgumentError('is too large')
  return number
}

export function parseDuration(value: string): number {
  const match = /^([1-9]\d*)([smhd]?)$/.exec(value)
  if (!match) throw new InvalidOptionArgumentError('must be a positive duration such as 30m, 12h, or 7d')
  const amount = Number(match[1])
  const multiplier = { '': 1, s: 1, m: 60, h: 3_600, d: 86_400 }[match[2]!]!
  const seconds = amount * multiplier
  if (!Number.isSafeInteger(seconds)) throw new InvalidOptionArgumentError('is too large')
  return seconds
}

export function parseFormat(value: string): 'plain' | 'source' | 'markdown' {
  if (value === 'plain' || value === 'source' || value === 'markdown') return value
  throw new InvalidOptionArgumentError('must be plain, source, or markdown')
}

export const parseNumber = parsePositiveInteger
