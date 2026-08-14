import { expect, test } from '@playwright/test'
import { Files, getFileChecksum } from '../../files'
import { checkLinkForDownload, createNoteSuccessfully, reveal } from '../../utils'

test.describe('web file flow', () => {
  test('encrypts file metadata and downloads identical bytes', async ({ page }) => {
    const checksum = await getFileChecksum(Files.PDF)
    const link = await createNoteSuccessfully(page, { text: 'Attached document', files: [Files.PDF] })
    await checkLinkForDownload(page, { link, checksum })
  })

  test('shows only safe local previews after reveal', async ({ page }) => {
    const link = await createNoteSuccessfully(page, { files: [Files.Image], password: 'blue-cat' })
    await reveal(page, link, 'blue-cat')
    await expect(page.getByTestId('revealed-files')).toContainText('image.jpg')
    const toggle = page.getByTestId('revealed-files').getByRole('button', { name: /preview/i })
    await expect(toggle).toHaveAttribute('aria-expanded', 'false')
    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-expanded', 'true')
    const preview = page.getByTestId('revealed-files').locator('img')
    await expect(preview).toBeVisible()
    expect(await preview.getAttribute('src')).toMatch(/^blob:/)
    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-expanded', 'false')
    await expect(preview).toHaveCount(0)
  })
})
