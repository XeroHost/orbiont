import assert from 'node:assert/strict'
import {
	cpSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	symlinkSync,
	unlinkSync,
	writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

import { verifyGlibBackport } from './glib-backport-integrity.mjs'

const root = fileURLToPath(new URL('../', import.meta.url))
const metadata = (directory) => ({
	packages: [
		{
			id: 'local-glib',
			name: 'glib',
			version: '0.18.5',
			source: null,
			manifest_path: join(directory, 'patches/glib-0.18.5/Cargo.toml'),
		},
	],
	resolve: { nodes: [{ id: 'local-glib' }] },
})

test('the reviewed source passes with a resolved local package', () => {
	assert.equal(verifyGlibBackport(root, metadata(root)), true)
})

for (const [name, mutate] of [
	[
		'upstream vulnerable pointer returns',
		(directory) => {
			const file = join(directory, 'patches/glib-0.18.5/src/variant_iter.rs')
			writeFileSync(file, readFileSync(file, 'utf8').replace('&mut p,', '&p,'))
		},
	],
	[
		'an unrelated source file changes',
		(directory) => writeFileSync(join(directory, 'patches/glib-0.18.5/src/lib.rs'), '// changed'),
	],
	[
		'a source file disappears',
		(directory) => unlinkSync(join(directory, 'patches/glib-0.18.5/src/lib.rs')),
	],
	[
		'an unexpected source file appears',
		(directory) => writeFileSync(join(directory, 'patches/glib-0.18.5/extra.rs'), '// extra'),
	],
	[
		'the integrity baseline changes',
		(directory) => writeFileSync(join(directory, 'patches/glib-0.18.5.integrity.json'), '{}'),
	],
	[
		'a source directory is linked',
		(directory) =>
			symlinkSync(
				join(root, 'patches/glib-0.18.5/src'),
				join(directory, 'patches/glib-0.18.5/linked-src'),
				'junction',
			),
	],
]) {
	test(`rejects when ${name}`, () => {
		const directory = mkdtempSync(join(tmpdir(), 'orbiont-glib-guard-'))
		try {
			mkdirSync(join(directory, 'patches'))
			cpSync(join(root, 'patches/glib-0.18.5'), join(directory, 'patches/glib-0.18.5'), {
				recursive: true,
			})
			cpSync(
				join(root, 'patches/glib-0.18.5.integrity.json'),
				join(directory, 'patches/glib-0.18.5.integrity.json'),
			)
			mutate(directory)
			assert.throws(() => verifyGlibBackport(directory, metadata(directory)), /GLib/)
		} finally {
			assert.ok(directory.startsWith(join(tmpdir(), 'orbiont-glib-guard-')))
			rmSync(directory, { recursive: true, force: true })
		}
	})
}

for (const [name, mutate] of [
	['a registry package', (graph) => (graph.packages[0].source = 'registry+https://crates.io')],
	['another version', (graph) => (graph.packages[0].version = '0.18.4')],
	['a duplicate GLib package', (graph) => graph.packages.push({ ...graph.packages[0] })],
	['an unresolved package', (graph) => (graph.resolve.nodes = [])],
	['another path', (graph) => (graph.packages[0].manifest_path = join(root, 'Cargo.toml'))],
]) {
	test(`rejects ${name} in the Cargo graph`, () => {
		const graph = metadata(root)
		mutate(graph)
		assert.throws(() => verifyGlibBackport(root, graph), /Cargo/)
	})
}
