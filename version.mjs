#!/usr/bin/env node

import shelljs from 'shelljs'
import { execSync } from 'node:child_process'

const VERSION = process.argv[2]
// https://semver.org/#is-there-a-suggested-regular-expression-regex-to-check-a-semver-string
const semver =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$/gm
if (!semver.test(VERSION)) {
  console.error('Invalid version number')
  process.exit(1)
}

// CLI and backend stay on one release version.
shelljs.sed('-i', /"version": ".*"/, `"version": "${VERSION}"`, './packages/cli/package.json')
shelljs.sed('-i', /^version = ".*"$/m, `version = "${VERSION}"`, './packages/backend/Cargo.toml')
execSync('cargo check -p nyanbin', { cwd: './packages/backend', stdio: 'inherit' })
