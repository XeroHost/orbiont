import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { after, before, test } from 'node:test'

import ts from 'typescript'
import { createServer } from 'vite'

let server, guard, operations, documents, management, bedrock
const previousWindow = globalThis.window
let calls = [],
	native

before(async () => {
	globalThis.window = {
		__TAURI_INTERNALS__: {
			invoke: async (command, args) => {
				calls.push({ command, args })
				return native(command, args)
			},
		},
	}
	server = await createServer({
		configFile: false,
		root: resolve(import.meta.dirname, '..'),
		optimizeDeps: { noDiscovery: true },
		server: { middlewareMode: true, hmr: false },
		appType: 'custom',
	})
	guard = await server.ssrLoadModule('/src/helpers/draft-exit.ts')
	operations = await server.ssrLoadModule('/src/helpers/bedrock-operations.ts')
	documents = await server.ssrLoadModule('/src/helpers/java-documents.ts')
	management = await server.ssrLoadModule('/src/helpers/local-management.ts')
	bedrock = await server.ssrLoadModule('/src/helpers/bedrock.ts')
})
after(async () => {
	await server?.close()
	if (previousWindow === undefined) delete globalThis.window
	else globalThis.window = previousWindow
})
test('navigation and window close share one pending decision and cancellation retains the draft', async () => {
	let decide,
		count = 0
	const exit = guard.createDraftExit(
		() => true,
		() => {
			count++
			return new Promise((resolve) => {
				decide = resolve
			})
		},
	)
	const navigation = exit.confirm()
	const windowClose = exit.confirm()
	assert.equal(count, 1)
	decide(false)
	assert.deepEqual(await Promise.all([navigation, windowClose]), [false, false])
	const next = exit.confirm()
	decide(true)
	assert.equal(await next, true)
})
test('clean documents leave without a prompt; failed confirmation remains blocked', async () => {
	assert.equal(
		await guard
			.createDraftExit(
				() => false,
				() => {
					throw Error('unexpected')
				},
			)
			.confirm(),
		true,
	)
	assert.equal(
		await guard
			.createDraftExit(
				() => true,
				async () => {
					throw Error('save failed')
				},
			)
			.confirm(),
		false,
	)
})
test('bulk removal invalidates once even when the second removal fails', async () => {
	const deleted = [],
		invalidated = []
	await assert.rejects(
		operations.removeBedrockBatch(
			[1, 2, 3],
			async (id) => {
				deleted.push(id)
				if (id === 2) throw Error('conflict')
			},
			async () => invalidated.push(true),
		),
		/conflict/,
	)
	assert.deepEqual(deleted, [1, 2])
	assert.deepEqual(invalidated, [true])
})
test('diagnostic export allowlists state/version and omits paths, diagnostics and credentials', () => {
	const report = operations.bedrockDiagnosticReport({
		supported: true,
		game_running: false,
		game: { version: '1.21', can_launch: true },
		launcher: null,
		launch: { state: 'timeout', elapsed_ms: 60000, diagnostic: 'C:\\Users\\secret token=abcdef' },
		token: 'secret',
	})
	assert.equal(report.launch.state, 'timeout')
	assert.equal(JSON.stringify(report).includes('secret'), false)
	assert.equal(JSON.stringify(report).includes('token'), false)
})

test('Java writes use read revision; external conflicts and failed saves retain revision and never cross instances', async () => {
	let instance = 'one',
		fail = true
	calls = []
	native = async (command) => {
		if (command.endsWith('file_read_document')) return { content: 'original', revision: 'r1' }
		if (fail) throw 'Document changed externally'
		return { revision: 'r2', recoveryId: 'safe-copy' }
	}
	const session = documents.createJavaDocuments(() => instance)
	assert.equal(await session.read('/options.txt'), 'original')
	await assert.rejects(session.write('/options.txt', 'draft'))
	fail = false
	await session.write('/options.txt', 'draft')
	assert.equal(calls.at(-1).args.expectedRevision, 'r1')
	await session.write('/options.txt', 'new draft')
	assert.equal(calls.at(-1).args.expectedRevision, 'r2')
	instance = 'two'
	const count = calls.length
	await assert.rejects(session.write('/options.txt', 'draft'), /not been read/)
	assert.equal(calls.length, count)
})
test('Java preview supplies current revision for restoration and storage removes only explicit valid copies', async () => {
	calls = []
	native = async (command) =>
		command.endsWith('file_preview_recovery')
			? { content: '<script>unsafe</script>', currentRevision: 'current-r', path: 'config.json' }
			: undefined
	const adapter = management.createJavaManagementAdapter(
		() => 'one',
		async () => {},
	)
	const copy = {
		id: 'copy',
		rootId: 'one',
		path: 'config.json',
		created: 1,
		size: 2,
		limited: false,
		state: 'available',
		removable: true,
	}
	const preview = await adapter.preview(copy)
	await adapter.restore(preview)
	assert.equal(calls.at(-1).args.expectedRevision, 'current-r')
	await adapter.remove([{ ...copy, category: 'recovery' }])
	assert.deepEqual(calls.at(-1).args.recoveryIds, ['copy'])
	const count = calls.length
	await assert.rejects(adapter.remove([{ ...copy, category: 'content' }]), /Invalid storage/)
	assert.equal(calls.length, count)
})
test('Bedrock recovery pages preserve state/size and removal rejects content even if marked removable', async () => {
	calls = []
	native = async (command) =>
		command.endsWith('list_recoveries_page')
			? {
					items: [
						{
							id: 'x',
							root_id: 'root',
							path: 'world',
							saved_at: 1,
							size_bytes: 9,
							size_limited: true,
							state: 'rollback_failed',
						},
					],
					total: 103,
					limited: true,
				}
			: undefined
	const adapter = management.createBedrockManagementAdapter(async () => {})
	const page = await adapter.list(100)
	assert.equal(page.entries[0].state, 'rollback_failed')
	assert.equal(page.entries[0].size, 9)
	assert.equal(calls.at(-1).args.offset, 100)
	assert.equal(calls.at(-1).args.limit, 100)
	await assert.rejects(
		adapter.remove([{ ...page.entries[0], category: 'content', removable: true }]),
		/Invalid storage/,
	)
})
test('Bedrock polling is restricted to process state; full snapshots are manual/focus/mutation driven', () => {
	assert.equal(bedrock.bedrockStatusQueryOptions().refetchInterval, undefined)
})

test('native draft exit can complete the approved close for the local main window', async () => {
	const capability = JSON.parse(
		await readFile(new URL('../../app/capabilities/core.json', import.meta.url), 'utf8'),
	)
	assert.equal(capability.local, true)
	assert.deepEqual(capability.windows, ['main'])
	assert.ok(capability.permissions.includes('core:window:allow-destroy'))
})

test('actual platform guard blocks instance/edition navigation and window close, then releases listeners', async () => {
	const source = await readFile(
		new URL('../src/composables/use-editor-exit.ts', import.meta.url),
		'utf8',
	)
	const javascript = ts.transpileModule(source, {
		compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
	}).outputText
	let mounted, unmounted, routeGuard, closeGuard, settle
	let prompts = 0,
		closes = 0,
		releasedRoute = 0,
		releasedClose = 0
	let failClose = true
	const events = new Map()
	const currentWindow = {
		onCloseRequested: async (callback) => {
			// Match Tauri's listener: approved requests destroy the native window.
			closeGuard = async (event) => {
				let prevented = false
				await callback({
					preventDefault() {
						prevented = true
						event.preventDefault()
					},
				})
				if (!prevented) await currentWindow.destroy()
			}
			return () => {
				releasedClose++
			}
		},
		destroy: async () => {
			closes++
			if (failClose) throw Error('native close rejected')
		},
	}
	const module = { exports: {} }
	const require = (id) => {
		if (id === 'vue')
			return {
				onMounted: (callback) => {
					mounted = callback
				},
				onUnmounted: (callback) => {
					unmounted = callback
				},
			}
		if (id === 'vue-router')
			return {
				useRouter: () => ({
					beforeEach: (callback) => {
						routeGuard = callback
						return () => {
							releasedRoute++
						}
					},
				}),
			}
		if (id.includes('draft-exit')) return guard
		return { getCurrentWindow: () => currentWindow }
	}
	new Function('require', 'module', 'exports', javascript)(require, module, module.exports)
	globalThis.window.addEventListener = (name, callback) => events.set(name, callback)
	globalThis.window.removeEventListener = (name) => events.delete(name)
	module.exports.useEditorExit(
		() => true,
		() => {
			prompts++
			return new Promise((resolve) => {
				settle = resolve
			})
		},
	)
	await mounted()
	const blockedNavigation = routeGuard({ params: { id: 'next-instance' }, path: '/bedrock' })
	let prevented = 0
	closeGuard({
		preventDefault() {
			prevented++
		},
	})
	closeGuard({
		preventDefault() {
			prevented++
		},
	})
	assert.equal(prompts, 1)
	assert.equal(prevented, 2)
	settle(false)
	assert.equal(await blockedNavigation, false)
	await new Promise(setImmediate)
	assert.equal(closes, 0)
	const unload = {
		preventDefault() {
			prevented++
		},
		returnValue: undefined,
	}
	events.get('beforeunload')(unload)
	assert.equal(unload.returnValue, '')
	closeGuard({
		preventDefault() {
			prevented++
		},
	})
	settle(true)
	const errors = []
	const previousError = console.error
	console.error = (message) => errors.push(message)
	try {
		await new Promise(setImmediate)
	} finally {
		console.error = previousError
	}
	assert.equal(closes, 1)
	assert.equal(errors.length, 1)
	const afterFailedClose = {
		preventDefault() {},
		returnValue: undefined,
	}
	events.get('beforeunload')(afterFailedClose)
	assert.equal(afterFailedClose.returnValue, '')
	failClose = false
	closeGuard({ preventDefault() {} })
	settle(true)
	await new Promise(setImmediate)
	assert.equal(closes, 2)
	// beforeunload can arrive after the destroy IPC response; approval must survive it.
	let preventedApprovedUnload = false
	const approvedUnload = {
		preventDefault() {
			preventedApprovedUnload = true
		},
		returnValue: undefined,
	}
	events.get('beforeunload')(approvedUnload)
	assert.equal(preventedApprovedUnload, false)
	assert.equal(approvedUnload.returnValue, undefined)
	assert.equal(closes, 2)
	assert.equal(prompts, 3)
	unmounted()
	assert.equal(releasedRoute, 1)
	assert.equal(releasedClose, 1)
	assert.equal(events.size, 0)
})
