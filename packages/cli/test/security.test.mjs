import assert from 'node:assert/strict'
import { once } from 'node:events'
import { createServer } from 'node:http'
import { mkdtemp, readFile, rename, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { PassThrough, Writable } from 'node:stream'
import { spawn } from 'node:child_process'
import test from 'node:test'

import {
  decodeBase64Url,
  decryptPayload,
  encodeBase64Url,
  escapeTerminal,
  getStdin,
  readPrivateFiles,
  saveFiles,
  validatePayload,
  writeNoteText,
} from '../dist/index.js'

function privateFile(name, contents = 'nya', type = 'text/plain') {
  const bytes = Buffer.from(contents)
  return { name, type, size: bytes.length, data: encodeBase64Url(bytes) }
}
function payload(files, text = '') { return { kind: 'text', format: 'plain', text, files } }
async function temporaryDirectory(t) {
  const directory = await mkdtemp(join(tmpdir(), 'nyanbin-cli-'))
  t.after(() => rm(directory, { recursive: true, force: true }))
  return directory
}
async function runCli(arguments_, inputChunks = []) {
  const child = spawn(process.execPath, [resolve('dist/cli.cjs'), ...arguments_], { cwd: resolve('.'), stdio: ['pipe', 'pipe', 'pipe'] })
  const stdout = [], stderr = []
  child.stdout.on('data', (chunk) => stdout.push(chunk))
  child.stderr.on('data', (chunk) => stderr.push(chunk))
  for (const { delay, chunk } of inputChunks) {
    await new Promise((resolveDelay) => setTimeout(resolveDelay, delay))
    child.stdin.write(chunk)
  }
  child.stdin.end()
  const [code] = await once(child, 'exit')
  return { code, stdout: Buffer.concat(stdout).toString(), stderr: Buffer.concat(stderr).toString() }
}

test('password stdin waits for EOF and preserves chunks except one final line ending', async () => {
  const input = new PassThrough()
  const bytes = Buffer.from('  密碼\ninside\t \r\n')
  const reading = getStdin(input)
  await new Promise((resolveDelay) => setTimeout(resolveDelay, 50))
  input.write(bytes.subarray(0, 3))
  input.write(bytes.subarray(3, 5))
  input.end(bytes.subarray(5))
  assert.equal(await reading, '  密碼\ninside\t ')
})

test('password stdin fails closed on stream errors', async () => {
  const input = new PassThrough()
  const reading = getStdin(input)
  input.write('partial')
  input.destroy(new Error('input failed'))
  await assert.rejects(reading, /input failed/)
})

test('delayed --password-stdin protects a created note with the exact intended factor', async (t) => {
  const id = '0123456789abcdefghijklmnopqrstuv'
  const deleteToken = encodeBase64Url(new Uint8Array(32).fill(7))
  const lifecycle = { expiresAt: Date.now() + 3_600_000, maxReads: 2 }
  let committed
  const server = createServer(async (request, response) => {
    const body = []
    for await (const chunk of request) body.push(chunk)
    response.setHeader('Content-Type', 'application/json')
    if (request.url === '/api/status') response.end(JSON.stringify({ protocol: 1, version: '1.0.0', limits: { maxEnvelopeBytes: 10_000_000, maxExpiresIn: 86_400, maxReads: 100 }, defaults: { expiresIn: 3600, maxReads: 2 }, capabilities: { files: true, passwords: true, formats: ['plain', 'source', 'markdown'] }, branding: { name: '', description: '', logoUrl: '', imprintUrl: '' } }))
    else if (request.url === '/api/notes/reserve') response.end(JSON.stringify({ id, deleteToken, lifecycle }))
    else if (request.url === `/api/notes/${id}` && request.method === 'PUT') { committed = JSON.parse(Buffer.concat(body).toString()); response.end(JSON.stringify({ id })) }
    else { response.statusCode = 404; response.end(JSON.stringify({ code: 'NOT_FOUND', message: 'not found' })) }
  })
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  t.after(() => new Promise((resolveClose) => server.close(resolveClose)))
  const address = server.address()
  const password = '  密碼\ninside\t '
  const bytes = Buffer.from(`${password}\r\n`)
  const result = await runCli(['create', 'text', 'secret', '--server', `http://127.0.0.1:${address.port}`, '--password-stdin'], [{ delay: 75, chunk: bytes.subarray(0, 4) }, { delay: 10, chunk: bytes.subarray(4) }])
  assert.equal(result.code, 0, result.stderr)
  assert.ok(committed)
  const noteUrl = /^Note: (.+)$/m.exec(result.stdout)?.[1]
  assert.ok(noteUrl)
  const secret = decodeBase64Url(new URL(noteUrl).hash.slice(1))
  assert.equal((await decryptPayload(committed.envelope, { id, lifecycle, secret, password })).text, 'secret')
  await assert.rejects(decryptPayload(committed.envelope, { id, lifecycle, secret }))
})

test('--password and --password-stdin are mutually exclusive', async () => {
  const result = await runCli(['create', 'text', 'secret', '--password', 'one', '--password-stdin'])
  assert.notEqual(result.code, 0)
  assert.match(result.stderr, /cannot be used with option/)
})

test('protocol accepts ordinary Unicode names and rejects terminal or non-portable metadata', () => {
  assert.doesNotThrow(() => validatePayload(payload([privateFile('猫-élève.txt')])))
  for (const name of ['', '.', '..', '.hidden', 'trailing.', ' leading', 'trailing ', 'dir/file', 'dir\\file', 'a:b', 'CON', 'nul.txt', 'COM1.log', 'LPT²', 'escape\u001b[2J.txt', 'c1\u0085.txt', 'line\u2028break.txt', 'spoof\u202Etxt.exe']) {
    assert.throws(() => validatePayload(payload([privateFile(name)])), (error) => error.code === 'INVALID_PAYLOAD', name)
  }
  assert.throws(() => validatePayload(payload([privateFile('safe.txt', 'nya', 'text/plain\u001b[2J')])), (error) => error.code === 'INVALID_PAYLOAD')
})

test('upload reads and validates through one no-follow descriptor', async (t) => {
  const directory = await temporaryDirectory(t)
  const source = join(directory, 'source.txt'), moved = join(directory, 'moved.txt'), secret = join(directory, 'secret.txt')
  await writeFile(source, Buffer.alloc(2_000_000, 0x41))
  await writeFile(secret, 'must not upload')
  const reading = readPrivateFiles([source])
  await rename(source, moved)
  await symlink(secret, source)
  let files
  try { files = await reading } catch (error) { assert.ok(['ELOOP', 'ENOENT'].includes(error.code), error) }
  if (files) {
    const contents = decodeBase64Url(files[0].data)
    assert.equal(Buffer.from(contents).includes(Buffer.from('must not upload')), false)
    assert.equal(contents.every((byte) => byte === 0x41), true)
  }
  await assert.rejects(readPrivateFiles([source]), (error) => error.code === 'ELOOP')
})

test('downloads never overwrite existing files or follow output symlinks', async (t) => {
  const directory = await temporaryDirectory(t)
  await writeFile(join(directory, 'report.txt'), 'existing')
  const [collision] = await saveFiles(payload([privateFile('report.txt', 'new')]), true, directory)
  assert.equal(await readFile(join(directory, 'report.txt'), 'utf8'), 'existing')
  assert.equal(await readFile(collision, 'utf8'), 'new')
  assert.equal(collision, join(directory, 'report (1).txt'))
  const target = join(directory, 'target.txt')
  await writeFile(target, 'target')
  await symlink(target, join(directory, 'link.txt'))
  const [besideLink] = await saveFiles(payload([privateFile('link.txt', 'safe')]), true, directory)
  assert.equal(await readFile(target, 'utf8'), 'target')
  assert.equal(await readFile(besideLink, 'utf8'), 'safe')
  assert.equal(besideLink, join(directory, 'link (1).txt'))
})

test('concurrent download name collisions use distinct exclusive files', async (t) => {
  const directory = await temporaryDirectory(t)
  const note = payload([privateFile('same.txt', 'contents')])
  const [[first], [second]] = await Promise.all([saveFiles(note, true, directory), saveFiles(note, true, directory)])
  assert.notEqual(first, second)
  assert.deepEqual(new Set([first, second]), new Set([join(directory, 'same.txt'), join(directory, 'same (1).txt')]))
})

test('terminal output is escaped by default and verbatim only in raw mode', () => {
  assert.equal(escapeTerminal('safe\u001b[2J\u202Ename'), 'safe\\u{001b}[2J\\u{202e}name')
  const chunks = []
  const output = new Writable({ write(chunk, _encoding, done) { chunks.push(Buffer.from(chunk)); done() } })
  writeNoteText('hello\u001b[2J', false, output)
  writeNoteText('raw\u001b[2J', true, output)
  assert.equal(Buffer.concat(chunks).toString(), 'hello\\u{001b}[2J\nraw\u001b[2J')
})
