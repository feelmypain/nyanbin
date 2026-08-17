import { expect, test } from '@playwright/test'
import { checkLinkForText, createNoteSuccessfully, reveal } from '../../utils'

test.describe('web short links', () => {
  test('notes without a password cannot mint a short link', async ({ page }) => {
    await createNoteSuccessfully(page, { text: 'No second factor, no guessable code.' })
    await expect(page.getByTestId('create-result')).toBeVisible()
    await expect(page.getByTestId('create-short')).toHaveCount(0)
    await expect(page.getByTestId('short-link')).toHaveCount(0)
  })

  test('password-protected notes mint a short link that resolves and decrypts', async ({ page }) => {
    const text = 'Short codes stay sealed behind the password.'
    const password = 'nyan-second-factor'
    await createNoteSuccessfully(page, { text, password })
    await page.getByTestId('create-short').click()
    const shortUrl = await page.getByTestId('short-link').inputValue()
    expect(shortUrl).toMatch(/^https?:\/\/[^\s]+\/s\/[0-9]{6}#[A-Za-z0-9_-]{43}$/)

    await page.goto('about:blank')
    await page.goto(shortUrl)
    await expect(page.getByTestId('reveal-gate')).toBeVisible()
    expect(page.url()).toMatch(/\/note\/[A-Za-z0-9]{32}#[A-Za-z0-9_-]{43}$/)
    await page.getByTestId('show-note-password').fill(password)
    await page.getByTestId('show-note-button').click()
    await expect(page.getByTestId('result')).toContainText(text)
  })

  test('a revoked note kills its short code', async ({ page }) => {
    await createNoteSuccessfully(page, { text: 'Dead notes take their codes along.', password: 'nyan-pw' })
    await page.getByTestId('create-short').click()
    const shortUrl = await page.getByTestId('short-link').inputValue()
    await page.getByTestId('revoke-button').click()
    await expect(page.getByTestId('create-result')).toContainText(/revoked/i)

    await page.goto('about:blank')
    await page.goto(shortUrl)
    await expect(page.getByRole('heading', { name: /points nowhere/i })).toBeVisible()
  })

  test('a consumed note kills its short code', async ({ page }) => {
    const text = 'One reveal, then the code dies too.'
    const password = 'nyan-pw'
    const link = await createNoteSuccessfully(page, { text, password, maxReads: 1 })
    await page.getByTestId('create-short').click()
    const shortUrl = await page.getByTestId('short-link').inputValue()

    await checkLinkForText(page, { link, text, password })

    await page.goto('about:blank')
    await page.goto(shortUrl)
    await expect(page.getByRole('heading', { name: /points nowhere/i })).toBeVisible()
  })

  test('malformed short codes are rejected locally', async ({ page }) => {
    await page.goto('/s/12ab56')
    await expect(page.getByRole('heading', { name: /malformed/i })).toBeVisible()
  })
})
