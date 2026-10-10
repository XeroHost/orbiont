import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve, dirname } from 'node:path'
import { spawnSync } from 'node:child_process'
import assert from 'node:assert/strict'
const original = JSON.parse((await import('node:fs')).readFileSync('turbo.jsonc', 'utf8'))
const fixture = mkdtempSync(join(tmpdir(), 'orbiont-ci-inputs-'))
try {
	writeFileSync(
		join(fixture, 'package.json'),
		JSON.stringify({
			name: 'ci-input-fixture',
			private: true,
			packageManager: 'pnpm@10.33.2',
			scripts: { lint: 'echo fixture' },
		}),
	)
	writeFileSync(join(fixture, 'pnpm-lock.yaml'), 'lockfileVersion: 9.0\n')
	writeFileSync(
		join(fixture, 'turbo.json'),
		JSON.stringify({ globalDependencies: original.globalDependencies, tasks: { lint: {} } }),
	)
	function hash() {
		const run = spawnSync(
			process.execPath,
			[resolve('node_modules/turbo/bin/turbo'), 'run', 'lint', '--dry=json'],
			{ cwd: fixture, encoding: 'utf8' },
		)
		assert.equal(run.status, 0, run.stderr)
		return JSON.parse(run.stdout).tasks[0].hash
	}
	const probes = [
		'Cargo.toml',
		'Cargo.lock',
		'packages/path-util/src/lib.rs',
		'packages/content-management/Cargo.toml',
		'patches/glib-0.18.5/src/collections.rs',
		'packages/app-lib/java/src/Main.java',
	]
	for (const file of probes) {
		const path = join(fixture, file)
		mkdirSync(join(path, '..'), { recursive: true })
		writeFileSync(path, 'first\n')
		const before = hash()
		writeFileSync(path, 'second\n')
		assert.notEqual(hash(), before, file)
		console.log(`PASS invalidates ${file}`)
	}
} finally {
	assert.equal(dirname(resolve(fixture)), resolve(tmpdir()))
	rmSync(fixture, { recursive: true, force: true })
}
