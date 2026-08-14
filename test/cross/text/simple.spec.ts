import { expect, test } from '@playwright/test'
import { CLI, checkLinkForText, createNoteSuccessfully, getLinkFromCLI } from '../../utils'

const text = 'Browser and CLI must decrypt the exact same authenticated envelope.'

test.describe('browser and CLI text interoperability', () => {
  test('CLI creates, browser reveals', async ({ page }) => {
    const created = await CLI('create', 'text', text, '--format', 'markdown', '--max-reads', '1')
    await checkLinkForText(page, { link: getLinkFromCLI(created.stdout), text })
  })

  test('browser creates, CLI opens', async ({ page }) => {
    const link = await createNoteSuccessfully(page, { text, format: 'source' })
    const opened = await CLI('open', link)
    expect(opened.stdout).toContain(text)
  })

  test('password mode works in both directions', async ({ page }) => {
    const password = 'shared-second-factor'
    const created = await CLI('create', 'text', text, '--password', password)
    await checkLinkForText(page, { link: getLinkFromCLI(created.stdout), text, password })

    const link = await createNoteSuccessfully(page, { text, password })
    const opened = await CLI('open', link, '--password', password)
    expect(opened.stdout).toContain(text)
  })
})
