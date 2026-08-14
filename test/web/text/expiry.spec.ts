import { expect, test } from '@playwright/test'
import { CLI, expectNoteMissing, getLinkFromCLI, parseNoteLink } from '../../utils'

test('absolute expiry removes an unread note', async () => {
  const created = await CLI('create', 'text', 'Short lived, but never scanner-consumed.', '--expires', '1s', '--max-reads', '10')
  const link = getLinkFromCLI(created.stdout)
  const { server, id } = parseNoteLink(link)
  const initial = await fetch(`${server}/api/notes/${id}`)
  expect(initial.status).toBe(200)
  expect((await initial.json()).lifecycle.remainingReads).toBe(10)

  const { promise, resolve } = Promise.withResolvers<void>()
  setTimeout(resolve, 1_500)
  await promise
  await expectNoteMissing(link)
})
