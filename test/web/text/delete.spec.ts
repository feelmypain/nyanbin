import { expect, test } from '@playwright/test'
import { createNoteSuccessfully, expectNoteMissing } from '../../utils'

test('creator can revoke using the locally held delete capability', async ({ page }) => {
  const link = await createNoteSuccessfully(page, { text: 'Creator-controlled revocation.' })
  await page.getByTestId('revoke-button').click()
  await expect(page.getByTestId('create-result')).toContainText(/revoked|deleted/i)
  await expectNoteMissing(link)
})
