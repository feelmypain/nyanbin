import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join, relative } from 'node:path'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const runJson = (command, args) => JSON.parse(execFileSync(command, args, {
  cwd: root,
  encoding: 'utf8',
  maxBuffer: 64 * 1024 * 1024,
}))

const rows = []
const add = (name, version, license, ecosystem, source) => {
  if (!name || !version || !license || !ecosystem || !source) {
    throw new Error(`Incomplete inventory row: ${JSON.stringify({ name, version, license, ecosystem, source })}`)
  }
  rows.push({ name, version, license, ecosystem, source })
}

const pnpmLicenses = runJson('pnpm', ['licenses', 'list', '--prod', '--long', '--json'])
for (const entries of Object.values(pnpmLicenses)) {
  for (const entry of entries) {
    for (const version of entry.versions) {
      const source = `https://www.npmjs.com/package/${entry.name}/v/${version}`
      add(entry.name, version, entry.license, 'pnpm', source)
    }
  }
}

for (const platform of ['x86_64-unknown-linux-musl', 'aarch64-unknown-linux-musl']) {
  const cargo = runJson('cargo', [
    'metadata', '--locked', '--format-version', '1',
    '--filter-platform', platform,
    '--manifest-path', 'packages/backend/Cargo.toml',
  ])
  const rootCrate = cargo.resolve.root
  const packageById = new Map(cargo.packages.map((pkg) => [pkg.id, pkg]))
  const nodeById = new Map(cargo.resolve.nodes.map((node) => [node.id, node]))
  const reachable = new Set()
  const visit = (id) => {
    if (reachable.has(id)) return
    reachable.add(id)
    for (const dependency of nodeById.get(id)?.deps ?? []) {
      const isProduction = dependency.dep_kinds.some(({ kind }) => kind === null)
      if (isProduction) visit(dependency.pkg)
    }
  }
  visit(rootCrate)
  for (const id of reachable) {
    if (id === rootCrate) continue
    const pkg = packageById.get(id)
    const source = pkg.source?.startsWith('registry+')
      ? `https://crates.io/crates/${encodeURIComponent(pkg.name)}/${encodeURIComponent(pkg.version)}`
      : pkg.source || pkg.repository || pkg.homepage
    add(pkg.name, pkg.version, pkg.license || `SEE LICENSE FILE ${pkg.license_file}`, 'cargo', source)
  }
}

const bundledAssets = [
  {
    name: 'Atkinson Hyperlegible font files',
    packageName: '@fontsource/atkinson-hyperlegible',
    license: 'OFL-1.1',
    source: 'https://github.com/googlefonts/atkinson-hyperlegible',
  },
  {
    name: 'M PLUS Rounded 1c font files',
    packageName: '@fontsource/m-plus-rounded-1c',
    license: 'OFL-1.1',
    source: 'https://github.com/coz-m/MPLUS_FONTS',
  },
  {
    name: 'Nyanbin favicon',
    version: `sha256:${createHash('sha256').update(readFileSync(join(root, 'packages/frontend/static/favicon.svg'))).digest('hex')}`,
    license: 'MIT',
    source: 'packages/frontend/static/favicon.svg',
  },
]
for (const asset of bundledAssets) {
  const dependency = asset.packageName
    ? rows.find((row) => row.ecosystem === 'pnpm' && row.name === asset.packageName)
    : null
  add(asset.name, asset.version || dependency?.version, asset.license, 'asset', asset.source)
}

rows.sort((a, b) =>
  a.ecosystem.localeCompare(b.ecosystem) ||
  a.name.localeCompare(b.name) ||
  a.version.localeCompare(b.version) ||
  a.source.localeCompare(b.source)
)
const unique = rows.filter((row, index) => index === 0 ||
  Object.keys(row).some((key) => row[key] !== rows[index - 1][key])
)
const csv = (value) => `"${String(value).replaceAll('"', '""')}"`
const output = [
  ['name', 'version', 'license', 'ecosystem', 'source'],
  ...unique.map(({ name, version, license, ecosystem, source }) => [name, version, license, ecosystem, source]),
].map((row) => row.map(csv).join(',')).join('\n') + '\n'
const destination = join(root, 'DEPENDENCY_LICENSES.csv')
writeFileSync(destination, output)
console.log(`Wrote ${unique.length} rows to ${relative(root, destination)}`)
