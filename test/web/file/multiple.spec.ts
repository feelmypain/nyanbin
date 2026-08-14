import { expect, test } from '@playwright/test'
import { Files, getFileChecksum } from '../../files'
import { createNoteSuccessfully, reveal } from '../../utils'

test('one reveal exposes multiple independently downloadable files', async ({ page }) => {
  const files = [Files.PDF, Files.Image]
  const checksums = await Promise.all(files.map(getFileChecksum))
  const link = await createNoteSuccessfully(page, { text: 'Two attachments', files })
  await reveal(page, link)
  await expect(page.getByTestId('revealed-files')).toContainText('AES.pdf')
  await expect(page.getByTestId('revealed-files')).toContainText('image.jpg')

  for (const [index, checksum] of checksums.entries()) {
    const [download] = await Promise.all([
      page.waitForEvent('download'),
      page.getByTestId(`download-file-${index}`).click(),
    ])
    const path = await download.path()
    if (!path) throw new Error('Download failed')
    expect(await getFileChecksum(path)).toBe(checksum)
  }
})
