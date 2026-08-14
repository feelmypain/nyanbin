import { build } from 'tsup'
import pkg from './package.json' with { type: 'json' }

const watch = process.argv.slice(2)[0] === '--watch'

await build({
  entry: [
    'src/index.ts',
    'src/cli.ts',
    'src/shared/shared.ts',
    'src/shared/api.ts',
    'src/shared/protocol.ts',
  ],
  dts: true,
  minify: true,
  format: ['esm', 'cjs'],
  target: 'node22',
  noExternal: ['mime', 'pretty-bytes'],
  clean: true,
  define: { VERSION: `"${pkg.version}"` },
  watch,
})
