import { expect, test } from '@playwright/test'
import { checkLinkForText, createNoteSuccessfully, expectNoteMissing, reveal } from '../../utils'

test.describe('web text flow', () => {
  test('creates and explicitly reveals literal text', async ({ page }) => {
    const text = 'Blue cats keep this <script>window.__nyanbin_xss = true</script> literal.'
    const link = await createNoteSuccessfully(page, { text, format: 'plain' })
    await checkLinkForText(page, { link, text })
    expect(await page.evaluate(() => '__nyanbin_xss' in window)).toBe(false)
  })

  test('requires the optional password second factor', async ({ page }) => {
    const text = 'The password is only a second factor.'
    const password = 'correct horse battery staple'
    const link = await createNoteSuccessfully(page, { text, password })
    await checkLinkForText(page, { link, text, password })
  })

  test('opens reveal separately and starts another note without revoking', async ({ page }) => {
    const text = 'Keep creator controls while the reveal page opens.'
    const link = await createNoteSuccessfully(page, { text })
    const revealPagePromise = page.waitForEvent('popup')
    await page.getByRole('link', { name: 'Open reveal page' }).click()
    const revealPage = await revealPagePromise

    await expect(page.getByTestId('revoke-button')).toBeVisible()
    await page.getByTestId('create-another').click()
    await expect(page.getByTestId('create-form')).toBeVisible()
    await expect(page.getByTestId('text-field')).toHaveValue('')

    await reveal(revealPage, link)
    await expect(revealPage.getByTestId('result')).toContainText(text)
  })

  test('a wrong password still consumes the reveal', async ({ page }) => {
    const link = await createNoteSuccessfully(page, {
      text: 'This read is spent before local authentication.',
      password: 'right-password',
      maxReads: 1,
    })
    await page.goto(link)
    await page.getByTestId('show-note-password').fill('wrong-password')
    await page.getByTestId('show-note-button').click()
    await expect(page.getByRole('alert')).toBeVisible()
    await expectNoteMissing(link)
  })

  for (const format of ['source', 'markdown'] as const) {
    test(`round-trips ${format} format`, async ({ page }) => {
      const text = format === 'source' ? 'const nyan = "sealed"\n' : '# Sealed\n\n**Still encrypted.**'
      const link = await createNoteSuccessfully(page, { text, format })
      await checkLinkForText(page, { link, text: format === 'source' ? 'sealed' : 'Sealed' })
    })
  }
})
