import { readFileSync, existsSync } from 'node:fs'
import { resolve } from 'node:path'
const architectures = ['aarch64-apple-darwin', 'x86_64-apple-darwin']
// Preflight for a future opt-in split workflow; never packages or signs unverified input.
export function verifyUniversalInputs(manifests, expected) {
	if (manifests.length !== 2) throw new Error('Both macOS architectures are required')
	for (const arch of architectures) {
		const matches = manifests.filter((entry) => entry.target === arch)
		if (matches.length !== 1) throw new Error(`Expected exactly one ${arch} artifact`)
		const entry = matches[0]
		for (const key of ['sha', 'configHash', 'frontendHash', 'features', 'profile']) {
			if (!expected[key] || entry[key] !== expected[key])
				throw new Error(`Incompatible ${key} for ${arch}`)
		}
		if (entry.signed !== false)
			throw new Error('Merge unsigned executables before signing/notarization')
	}
	return architectures.map((arch) => manifests.find((entry) => entry.target === arch))
}
if (process.argv[1]?.endsWith('ci-universal-inputs.mjs')) {
	const [expectedPath, ...paths] = process.argv.slice(2)
	const ordered = verifyUniversalInputs(
		paths.map((path) => JSON.parse(readFileSync(path, 'utf8'))),
		JSON.parse(readFileSync(expectedPath, 'utf8')),
	)
	for (const input of ordered) {
		if (!existsSync(resolve(input.binary))) throw new Error(`Missing ${input.target} executable`)
	}
	console.log(JSON.stringify(ordered, null, 2))
}
