import { createHash } from 'node:crypto'
import { lstatSync, readFileSync, readdirSync, realpathSync } from 'node:fs'
import { join, resolve } from 'node:path'

const manifestHash = '26a13173f59617782a4d701d5d9de4b0157a9e2efcdc56da16fd71905b0b56c7'
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')

// This pins every byte of the published crate plus the two-line upstream fix.
// A changed baseline requires reviewing the provenance documented alongside it.
export function verifyGlibBackport(root, metadata) {
	const vendor = resolve(root, 'patches/glib-0.18.5')
	const raw = readFileSync(join(root, 'patches/glib-0.18.5.integrity.json'))
	if (sha256(raw) !== manifestHash) throw new Error('GLib integrity baseline changed.')
	const { files } = JSON.parse(raw)
	const seen = []
	function visit(directory, prefix = '') {
		if (!lstatSync(directory).isDirectory()) throw new Error('GLib directory is not regular.')
		for (const entry of readdirSync(directory, { withFileTypes: true })) {
			const relative = prefix + entry.name
			const absolute = join(directory, entry.name)
			if (entry.isDirectory()) visit(absolute, relative + '/')
			else {
				if (!entry.isFile() || sha256(readFileSync(absolute)) !== files[relative]) {
					throw new Error(`GLib source integrity failed: ${relative}`)
				}
				seen.push(relative)
			}
		}
	}
	visit(vendor)
	if (seen.length !== Object.keys(files).length) throw new Error('GLib source files missing.')
	const packages = metadata.packages.filter((pkg) => pkg.name === 'glib')
	if (
		packages.length !== 1 ||
		packages[0].version !== '0.18.5' ||
		packages[0].source !== null ||
		realpathSync(packages[0].manifest_path) !== realpathSync(join(vendor, 'Cargo.toml')) ||
		!metadata.resolve?.nodes.some((node) => node.id === packages[0].id)
	) {
		throw new Error('Cargo must resolve only the reviewed local GLib 0.18.5 backport.')
	}
	return true
}
