import { expect, test } from '@playwright/test'
import { CLI, expectNoteMissing, getDeleteTokenFromCLI, getLinkFromCLI, parseNoteLink } from '../../utils'

test.describe('Nyanbin CLI text flow', () => {
  test('reports v1 server limits and defaults', async () => {
    const info = await CLI('info')
    expect(info.stdout).toContain('protocol')
    expect(info.stdout).toContain('max_envelope')
    expect(info.stdout).toContain('default_expiry_seconds')
  })

  test('creates and opens text with declared lifecycle flags', async () => {
    const text = 'CLI and browser share the same Nyanbin v1 envelope.'
    const created = await CLI('create', 'text', text, '--format', 'source', '--expires', '1h', '--max-reads', '2')
    const link = getLinkFromCLI(created.stdout)
    expect(getDeleteTokenFromCLI(created.stdout)).toMatch(/^[A-Za-z0-9_-]{43}$/)
    const opened = await CLI('open', link)
    expect(opened.stdout).toContain(text)
  })

  test('deletes with the independent creator capability', async () => {
    const created = await CLI('create', 'text', 'Revoke me')
    const link = getLinkFromCLI(created.stdout)
    const token = getDeleteTokenFromCLI(created.stdout)
    const { server, id } = parseNoteLink(link)
    const denied = await fetch(`${server}/api/notes/${id}`, {
      method: 'DELETE',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ deleteToken: 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA' }),
    })
    expect(denied.status).toBe(403)
    await CLI('delete', link, '--delete-token', token)
    await expectNoteMissing(link)
  })

  test('supports password as a second factor', async () => {
    const text = 'Both the fragment secret and password are required.'
    const created = await CLI('create', 'text', text, '--password', 'neko-neko')
    const opened = await CLI('open', getLinkFromCLI(created.stdout), '--password', 'neko-neko')
    expect(opened.stdout).toContain(text)
  })
})
