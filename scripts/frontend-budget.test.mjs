import assert from 'node:assert/strict'
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'

import { measureBundle } from './frontend-budget.mjs'

test('bundle measurements include nested JavaScript and separate Ace helpers', async () => {
	const fixture = await mkdtemp(path.join(tmpdir(), 'orbiont-bundle-budget-'))
	try {
		await mkdir(path.join(fixture, 'assets'))
		await writeFile(path.join(fixture, 'main.js'), 'export const x = 1\n')
		await writeFile(path.join(fixture, 'assets', 'mode-json.js'), 'x'.repeat(1000))
		await writeFile(path.join(fixture, 'assets', 'large.png'), 'x'.repeat(2000))
		await writeFile(path.join(fixture, 'assets', 'mode-json.js.map'), '{}')
		await writeFile(
			path.join(fixture, 'bundle-metrics.json'),
			JSON.stringify({ aceSourceBytes: 1200, aceModules: 2 }),
		)
		const metrics = await measureBundle(fixture)
		assert.equal(metrics.javascriptFiles, 2)
		assert.equal(metrics.javascriptBytes, 1019)
		assert.equal(metrics.aceAuxiliaryBytes, 1000)
		assert.equal(metrics.aceSourceBytes, 1200)
		assert.ok(metrics.gzipBytes < metrics.javascriptBytes)
	} finally {
		await rm(fixture, { recursive: true, force: true })
	}
})

test('a missing module report cannot silently pass the Ace budget', async () => {
	const fixture = await mkdtemp(path.join(tmpdir(), 'orbiont-bundle-budget-'))
	try {
		await assert.rejects(measureBundle(fixture), /ENOENT/)
	} finally {
		await rm(fixture, { recursive: true, force: true })
	}
})
