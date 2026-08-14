import { expect, test } from '@playwright/test'
import { checkLinkForText, createNoteSuccessfully, expectNoteMissing, parseNoteLink } from '../../utils'

test.describe('atomic read lifecycle', () => {
  test('passive navigation and info requests do not consume reads', async ({ page }) => {
    const text = 'Only an explicit reveal consumes this note.'
    const link = await createNoteSuccessfully(page, { text, maxReads: 2 })
    const { server, id } = parseNoteLink(link)

    await page.goto(link)
    await expect(page.getByTestId('reveal-gate')).toBeVisible()
    for (let index = 0; index < 3; index += 1) {
      const info = await fetch(`${server}/api/notes/${id}`)
      expect(info.status).toBe(200)
      expect((await info.json()).lifecycle.remainingReads).toBe(2)
    }

    await checkLinkForText(page, { link, text })
    const afterOneReveal = await fetch(`${server}/api/notes/${id}`)
    expect((await afterOneReveal.json()).lifecycle.remainingReads).toBe(1)
    await checkLinkForText(page, { link, text })
    await expectNoteMissing(link)
  })
})
