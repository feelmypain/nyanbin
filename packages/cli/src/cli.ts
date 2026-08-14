#!/usr/bin/env node

import { Argument, Option, program } from '@commander-js/extra-typings'
import prettyBytes from 'pretty-bytes'

import { deleteNoteByLink } from './actions/delete.js'
import { download, escapeTerminalLine } from './actions/download.js'
import { upload } from './actions/upload.js'
import { createAPI } from './shared/api.js'
import { parseDuration, parseFile, parseFormat, parsePositiveInteger, parseURL } from './utils/parsers.js'
import { getStdin } from './utils/stdin.js'
import { checkConstraints, errorMessage, exit } from './utils/utils.js'

const defaultServer = process.env['NYANBIN_SERVER'] || 'http://localhost:8000'
const server = new Option('-s, --server <url>', 'Nyanbin server URL').default(defaultServer)
const expires = new Option('-e, --expires <duration>', 'lifetime in seconds or with s/m/h/d suffix').argParser(parseDuration)
const maxReads = new Option('-r, --max-reads <number>', 'maximum successful reveals').argParser(parsePositiveInteger)
const password = new Option('-p, --password <string>', 'optional second-factor password').conflicts('passwordStdin')
const passwordStdin = new Option('--password-stdin', 'read the second-factor password from standard input').conflicts('password')
const format = new Option('-f, --format <format>', 'text format: plain, source, or markdown').argParser(parseFormat).default('plain' as const)
const attachments = new Option('--file <path>', 'attach a file (repeatable)').argParser(parseFile).default([] as string[])
const raw = new Option('--raw', 'write decrypted note text verbatim (unsafe on untrusted terminals)').default(false)
const all = new Option('-a, --all', 'save all files without prompting').default(false)
const deleteToken = new Option('-t, --delete-token <token>', 'creator delete token').makeOptionMandatory()
const link = new Argument('<url>', 'Nyanbin note URL').argParser(parseURL)

if (Number(process.versions.node.split('.')[0]) < 22) exit('Nyanbin requires Node.js 22 or newer')

// Injected by the package build.
// @ts-expect-error build-time constant
const version: string = VERSION

program.name('nyanbin').description('Create and open end-to-end encrypted Nyanbin notes').version(version)

program
  .command('info')
  .description('show server limits and defaults')
  .addOption(server)
  .action(async (options) => {
    const api = createAPI({ server: options.server })
    const response = await api.status()
    console.table({
      protocol: response.protocol,
      max_envelope: prettyBytes(response.limits.maxEnvelopeBytes),
      max_expiry_seconds: response.limits.maxExpiresIn,
      max_reads: response.limits.maxReads,
      default_expiry_seconds: response.defaults.expiresIn,
      default_reads: response.defaults.maxReads ?? 'unlimited',
    })
  })

const create = program.command('create').alias('send').description('create an encrypted note')

create
  .command('text')
  .description('create a text note, optionally with files')
  .addArgument(new Argument('<text>', 'note text'))
  .addOption(server)
  .addOption(expires)
  .addOption(maxReads)
  .addOption(password)
  .addOption(passwordStdin)
  .addOption(format)
  .addOption(attachments)
  .action(async (text, options) => {
    const api = createAPI({ server: options.server })
    const lifecycle = await checkConstraints({ expiresIn: options.expires, maxReads: options.maxReads }, api)
    const suppliedPassword = options.passwordStdin ? await getStdin() : options.password
    if (suppliedPassword !== undefined && suppliedPassword.length === 0) throw new Error('password must not be empty')
    const result = await upload(text, {
      ...lifecycle,
      format: options.format,
      files: options.file,
      ...(suppliedPassword === undefined ? {} : { password: suppliedPassword }),
    }, api)
    console.log(`Note: ${result.url}`)
    console.log(`Delete token: ${result.deleteToken}`)
  })

create
  .command('file')
  .description('create a note containing one or more files')
  .addArgument(new Argument('<file...>', 'files to encrypt').argParser(parseFile))
  .addOption(server)
  .addOption(expires)
  .addOption(maxReads)
  .addOption(password)
  .addOption(passwordStdin)
  .action(async (files, options) => {
    const api = createAPI({ server: options.server })
    const lifecycle = await checkConstraints({ expiresIn: options.expires, maxReads: options.maxReads }, api)
    const suppliedPassword = options.passwordStdin ? await getStdin() : options.password
    if (suppliedPassword !== undefined && suppliedPassword.length === 0) throw new Error('password must not be empty')
    const result = await upload(files, {
      ...lifecycle,
      ...(suppliedPassword === undefined ? {} : { password: suppliedPassword }),
    }, api)
    console.log(`Note: ${result.url}`)
    console.log(`Delete token: ${result.deleteToken}`)
  })

program
  .command('open')
  .description('reveal and decrypt a note')
  .addArgument(link)
  .addOption(password)
  .addOption(passwordStdin)
  .addOption(all)
  .addOption(raw)
  .action(async (url, options) => {
    const suppliedPassword = options.passwordStdin ? await getStdin() : options.password
    if (suppliedPassword !== undefined && suppliedPassword.length === 0) throw new Error('password must not be empty')
    await download(url, {
      all: options.all,
      raw: options.raw,
      ...(suppliedPassword === undefined ? {} : { password: suppliedPassword }),
    })
  })

program
  .command('delete')
  .description('delete a note using its creator token')
  .addArgument(link)
  .addOption(deleteToken)
  .action(async (url, options) => {
    await deleteNoteByLink(url, options.deleteToken)
    console.log('Note deleted.')
  })

program.parseAsync().catch((error: unknown) => {
  exit(escapeTerminalLine(errorMessage(error)))
})
