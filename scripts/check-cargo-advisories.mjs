import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

import { verifyGlibBackport } from './glib-backport-integrity.mjs'

const root = fileURLToPath(new URL('../', import.meta.url))

// sqlx's lockfile includes optional MySQL dependencies even though Orbiont only
// enables SQLite. Do not accept this exception if RSA enters any supported build.
const tree = spawnSync(
	'cargo',
	[
		'tree',
		'--locked',
		'--workspace',
		'--target',
		'all',
		'--all-features',
		'--edges',
		'normal,build',
		'--prefix',
		'none',
	],
	{ cwd: root, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 },
)
if (tree.error || tree.status !== 0) {
	process.stderr.write(tree.stderr || String(tree.error))
	process.exit(1)
}
if (/^rsa v/m.test(tree.stdout)) {
	console.error('RSA is reachable in the build graph. RUSTSEC-2023-0071 cannot be excepted.')
	process.exit(1)
}
console.log(
	'RUSTSEC-2023-0071: optional lockfile dependency; RSA absent from all-platform build graph.',
)
const metadata = spawnSync(
	'cargo',
	['metadata', '--locked', '--all-features', '--format-version', '1'],
	{
		cwd: root,
		encoding: 'utf8',
		maxBuffer: 32 * 1024 * 1024,
	},
)
if (metadata.error || metadata.status !== 0) {
	process.stderr.write(metadata.stderr || String(metadata.error))
	process.exit(1)
}
try {
	verifyGlibBackport(root, JSON.parse(metadata.stdout))
} catch (error) {
	console.error(error.message)
	process.exit(1)
}
console.log('RUSTSEC-2024-0429: reviewed upstream fix verified in the resolved local GLib source.')
const audit = spawnSync(
	'cargo',
	['audit', '--ignore', 'RUSTSEC-2023-0071', '--ignore', 'RUSTSEC-2024-0429'],
	{ cwd: root, stdio: 'inherit' },
)
if (audit.error) console.error(audit.error.message)
process.exit(audit.status ?? 1)
