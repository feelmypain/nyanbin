import { StringDecoder } from 'node:string_decoder'

import type { Readable } from 'node:stream'

export async function getStdin(input: Readable = process.stdin): Promise<string> {
  if ('isTTY' in input && input.isTTY === true) throw new Error('--password-stdin requires piped standard input')

  const chunks: Buffer[] = []
  for await (const chunk of input) chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk))

  const decoder = new StringDecoder('utf8')
  let password = decoder.write(Buffer.concat(chunks)) + decoder.end()
  if (password.endsWith('\r\n')) password = password.slice(0, -2)
  else if (password.endsWith('\n') || password.endsWith('\r')) password = password.slice(0, -1)
  return password
}
