#!/usr/bin/env node
// Renders docs/api/openapi.yaml to canonical JSON at packages/backend/openapi.json.
// The backend embeds the JSON artifact and serves it verbatim at /api/openapi.json.

import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse } from 'yaml'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const source = join(root, 'docs', 'api', 'openapi.yaml')
const target = join(root, 'packages', 'backend', 'openapi.json')

const document = parse(readFileSync(source, 'utf8'))
writeFileSync(target, `${JSON.stringify(document, null, 2)}\n`)
console.log(`Wrote ${target}`)
