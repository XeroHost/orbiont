import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { after, before, test } from 'node:test'

import ts from 'typescript'
import { createServer } from 'vite'
import * as vue from 'vue'
import { compileScript, parse } from 'vue/compiler-sfc'

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

test('pending Bedrock launch blocks management before process detection', () => {
	for (const state of ['requested', 'starting'])
		assert.equal(bedrock.isBedrockLaunchPending({ state }), true)
	for (const state of ['idle', 'running', 'timeout', 'failed'])
		assert.equal(bedrock.isBedrockLaunchPending({ state }), false)
	assert.equal(bedrock.isBedrockLaunchPending(undefined), false)
})

test('installed Bedrock blocks content mutations throughout a pending launch', async () => {
	const source = await readFile(
		new URL('../src/components/ui/bedrock/BedrockInstalled.vue', import.meta.url),
		'utf8',
	)
	const script = compileScript(parse(source).descriptor, { id: 'bedrock-pending-test' }).content
	const javascript = ts.transpileModule(script, {
		compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
	}).outputText
	const notices = new Proxy({}, { get: (_, key) => ({ defaultMessage: String(key) }) })
	let deletes = 0
	const require = (id) => {
		if (id === 'vue') return vue
		if (id === 'vue-router') return { useRoute: () => ({ query: {} }) }
		if (id === '@tanstack/vue-query')
			return {
				useQueryClient: () => ({ invalidateQueries: async () => {} }),
				useQuery: () => ({ data: vue.ref({ roots: [], items: [] }) }),
			}
		if (id.includes('bedrock-messages')) return { bedrockMessages: notices }
		if (id.includes('management-messages')) return { managementMessages: notices }
		if (id.includes('local-management')) return { createBedrockManagementAdapter: () => ({}) }
		if (id.includes('bedrock-operations')) return { removeBedrockBatch: async () => deletes++ }
		return {
			useVIntl: () => ({ formatMessage: (message) => message.defaultMessage }),
		}
	}
	const module = { exports: {} }
	new Function('require', 'module', 'exports', javascript)(require, module, module.exports)
	const props = vue.reactive({
		game: {},
		launcherAvailable: false,
		busy: false,
		running: false,
		launchPending: true,
	})
	const scope = vue.effectScope()
	try {
		const state = scope.run(() => module.exports.default.setup(props, { expose() {}, emit() {} }))
		assert.equal(state.manageBusy.value, true)
		await assert.rejects(state.deleteItems([]), /closeToManage/)
		assert.equal(deletes, 0)
		props.launchPending = false
		assert.equal(state.manageBusy.value, false)
		await state.deleteItems([])
		assert.equal(deletes, 1)
		props.running = true
		await assert.rejects(state.deleteItems([]), /closeToManage/)
		assert.equal(deletes, 1)
	} finally {
		scope.stop()
	}
})
