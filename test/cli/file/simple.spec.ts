import { expect, test } from '@playwright/test'
import { mkdtemp, rm } from 'node:fs/promises'
import { basename, join } from 'node:path'
import { tmpdir } from 'node:os'
import { Files, getFileChecksum } from '../../files'
import { CLI, CLIAt, getLinkFromCLI } from '../../utils'

test('CLI round-trips multiple file bytes', async () => {
  const expected = await Promise.all([Files.Image, Files.PDF].map(getFileChecksum))
  const created = await CLI('create', 'file', Files.Image, Files.PDF, '--max-reads', '1')
  const output = await mkdtemp(join(tmpdir(), 'nyanbin-e2e-'))
  try {
    await CLIAt(output, 'open', getLinkFromCLI(created.stdout), '--all')
    expect(await getFileChecksum(join(output, basename(Files.Image)))).toBe(expected[0])
    expect(await getFileChecksum(join(output, basename(Files.PDF)))).toBe(expected[1])
  } finally {
    await rm(output, { recursive: true, force: true })
  }
})
