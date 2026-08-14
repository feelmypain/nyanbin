import { expect, test } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { Files } from '../../files'

test('pasted image becomes an encrypted attachment', async ({ page, browserName }) => {
  test.skip(browserName !== 'chromium', 'Constructing clipboard files is supported by Chromium')
  const data = (await readFile(Files.Image)).toString('base64')
  await page.goto('/')
  await expect(page.getByTestId('create-form')).toBeVisible()
  await page.getByTestId('create-form').evaluate((form, base64) => {
    const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0))
    const transfer = new DataTransfer()
    transfer.items.add(new File([bytes], 'pasted-image.jpg', { type: 'image/jpeg' }))
    form.dispatchEvent(new ClipboardEvent('paste', { bubbles: true, clipboardData: transfer }))
  }, data)
  await expect(page.getByTestId('attachment-list')).toContainText('pasted-image.jpg')
  await page.getByTestId('create-button').click()
  await expect(page.getByTestId('create-result')).toBeVisible()
})
