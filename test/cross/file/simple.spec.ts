import { expect, test } from '@playwright/test'
import { mkdtemp, rm } from 'node:fs/promises'
import { basename, join } from 'node:path'
import { tmpdir } from 'node:os'
import { Files, getFileChecksum } from '../../files'
import { CLI, CLIAt, checkLinkForDownload, createNoteSuccessfully, getLinkFromCLI } from '../../utils'

test.describe('browser and CLI file interoperability', () => {
  test('CLI creates, browser downloads', async ({ page }) => {
    const created = await CLI('create', 'file', Files.Image)
    await checkLinkForDownload(page, {
      link: getLinkFromCLI(created.stdout),
      checksum: await getFileChecksum(Files.Image),
    })
  })

  test('browser creates, CLI downloads', async ({ page }) => {
    const link = await createNoteSuccessfully(page, { files: [Files.Image] })
    const output = await mkdtemp(join(tmpdir(), 'nyanbin-cross-'))
    try {
      await CLIAt(output, 'open', link, '--all')
      expect(await getFileChecksum(join(output, basename(Files.Image)))).toBe(await getFileChecksum(Files.Image))
    } finally {
      await rm(output, { recursive: true, force: true })
    }
  })
})
