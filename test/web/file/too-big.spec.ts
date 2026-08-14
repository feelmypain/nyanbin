import { expect, test } from '@playwright/test'

test('rejects an attachment beyond the configured envelope limit', async ({ page }) => {
  test.setTimeout(90_000)
  let reserveRequests = 0
  page.on('request', (request) => {
    if (new URL(request.url()).pathname === '/api/notes/reserve') reserveRequests += 1
  })
  await page.goto('/')
  await page.getByTestId('file-upload').setInputFiles({
    name: 'too-large.bin',
    mimeType: 'application/octet-stream',
    buffer: Buffer.alloc(10 * 1024 * 1024 + 1),
  })
  await expect(page.getByTestId('create-button')).toBeDisabled()
  await expect(page.getByTestId('create-result')).toHaveCount(0)
  await expect(page.getByRole('alert')).toContainText(/over by|exceeds/i)
  expect(reserveRequests).toBe(0)
})
