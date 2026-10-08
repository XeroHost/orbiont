import assert from 'node:assert/strict'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'

import { loadEnv } from 'vite'

import { publicEnvPrefixes } from '../public-env.ts'

test('Vite exposes public settings without signing or provider credentials', async () => {
	const directory = await mkdtemp(path.join(os.tmpdir(), 'orbiont-public-env-'))
	try {
		await writeFile(
			path.join(directory, '.env'),
			[
				'TAURI_SIGNING_PRIVATE_KEY=test-private-key',
				'TAURI_SIGNING_PRIVATE_KEY_PASSWORD=test-password',
				'ORBIONT_API_KEY=test-provider-key',
				'MODRINTH_ACCESS_TOKEN=test-access-token',
				'VITE_PRIVATE_KEY=test-legacy-prefix',
				'ORBIONT_UPDATES_URL=https://example.test/updates.json',
				'MODRINTH_URL=https://example.test',
				'MODRINTH_API_BASE_URL=https://example.test/api',
				'VITE_VUE_SCAN=true',
			].join('\n'),
		)
		const exposed = loadEnv('test', directory, publicEnvPrefixes)
		for (const key of [
			'TAURI_SIGNING_PRIVATE_KEY',
			'TAURI_SIGNING_PRIVATE_KEY_PASSWORD',
			'ORBIONT_API_KEY',
			'MODRINTH_ACCESS_TOKEN',
			'VITE_PRIVATE_KEY',
		]) {
			assert.equal(Object.hasOwn(exposed, key), false, key)
		}
		for (const key of publicEnvPrefixes) assert.equal(typeof exposed[key], 'string', key)
	} finally {
		await rm(directory, { recursive: true, force: true })
	}
})
