import { copyFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const cli = join(root, 'packages', 'cli')
const artifacts = ['LICENSE', 'THIRD_PARTY_NOTICES', 'DEPENDENCY_LICENSES.csv']

for (const artifact of artifacts) copyFileSync(join(root, artifact), join(cli, artifact))
