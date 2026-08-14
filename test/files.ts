import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'

export const Files = {
  PDF: 'test/assets/AES.pdf',
  Image: 'test/assets/image.jpg',
}

export async function getFileChecksum(file: string) {
  const buffer = await readFile(file)
  const hash = createHash('sha3-256').update(buffer).digest('hex')
  return hash
}

