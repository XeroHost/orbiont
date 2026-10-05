import assert from 'node:assert/strict'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { after, before, test } from 'node:test'

import { createServer } from 'vite'

let server
let bedrock
let calls = []
let selected = null
const previousWindow = globalThis.window
before(async () => {
	globalThis.window = {
		__TAURI_INTERNALS__: {
			invoke: async (command, args) => {
				calls.push({ command, args })
				if (command === 'plugin:dialog|open') return selected
				return { supported: true, game: null, launcher: null }
			},
		},
	}
	server = await createServer({
		configFile: false,
		root: resolve(import.meta.dirname, '..'),
		cacheDir: join(tmpdir(), 'orbiont-bedrock-tests'),
		optimizeDeps: { noDiscovery: true, include: [] },
		server: { middlewareMode: true, hmr: false },
		appType: 'custom',
	})
	bedrock = await server.ssrLoadModule('/src/helpers/bedrock.ts')
})
after(async () => {
	await server?.close()
	if (previousWindow === undefined) delete globalThis.window
	else globalThis.window = previousWindow
})

test('canceling the file picker does not launch Minecraft', async () => {
	calls = []
	selected = null
	assert.equal(await bedrock.pickAndImportBedrockFile('Contenido Bedrock'), false)
	assert.equal(calls.length, 1)
	assert.equal(calls[0].command, 'plugin:dialog|open')
	assert.deepEqual(calls[0].args.options.filters, [
		{ name: 'Contenido Bedrock', extensions: ['mcworld', 'mcpack', 'mcaddon'] },
	])
})

test('the selected file goes only to the Bedrock importer without a Java instance', async () => {
	calls = []
	selected = 'C:\\Downloads\\Mundo español.MCWORLD'
	assert.equal(await bedrock.pickAndImportBedrockFile('Contenido Bedrock'), true)
	assert.deepEqual(calls[1], {
		command: 'plugin:bedrock|import_file',
		args: { path: selected },
	})
	assert.equal(calls.length, 2)
})

test('a failed native import is not reported as submitted', async () => {
	selected = 'C:\\Downloads\\broken.mcpack'
	globalThis.window.__TAURI_INTERNALS__.invoke = async (command) => {
		if (command === 'plugin:dialog|open') return selected
		throw { code: 'invalid_file', message: 'Not a ZIP' }
	}
	await assert.rejects(bedrock.pickAndImportBedrockFile('Contenido Bedrock'), {
		code: 'invalid_file',
	})
})
