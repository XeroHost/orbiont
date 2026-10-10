import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import { CancelledError, isCancelledError, QueryClient } from '@tanstack/vue-query'
import ts from 'typescript'
import * as vue from 'vue'

const source = readFileSync(
	new URL('../src/composables/use-instance-metadata-refresh.ts', import.meta.url),
	'utf8',
)
const code = ts.transpileModule(source, {
	compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText
const listKey = ['instances', 'list']
function fixture(t, queryFn) {
	const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
	const handlers = new Map()
	const scope = vue.effectScope()
	const exports = {}
	const imports = {
		vue,
		'@tanstack/vue-query': { useQueryClient: () => client, isCancelledError },
		'@/helpers/install': {
			installJobInstanceId: () => null,
			isInstallJobFinished: () => false,
		},
		'@/helpers/synced-packs': { syncedPackKeys: { all: ['synced-packs'] } },
		'@/pages/instance/query-options': {
			instanceKeys: { list: () => listKey, detail: (id) => ['instances', 'summary', id] },
			instanceListQueryOptions: () => ({ queryKey: listKey, queryFn }),
		},
		'./use-app-event': { useAppEvent: (type, handler) => handlers.set(type, handler) },
	}
	new Function('require', 'exports', code)((name) => {
		assert.ok(name in imports, name)
		return imports[name]
	}, exports)
	scope.run(() => exports.useInstanceMetadataRefresh({}))
	t.after(() => {
		scope.stop()
		client.clear()
	})
	return { client, emit: (payload) => handlers.get('instance')(payload), stop: () => scope.stop() }
}

test('a replacement refetch does not turn instance events into CancelledError', async (t) => {
	let started
	const pending = new Promise((resolve) => {
		started = resolve
	})
	let calls = 0
	const state = fixture(t, async () => {
		if (++calls === 1) {
			started()
			return new Promise(() => {})
		}
		return [{ id: 'one', name: 'Updated instance' }]
	})
	state.client.setQueryData(listKey, [{ id: 'one', name: 'Old instance' }])
	const event = state.emit({ instance_id: 'one', event: 'edited' })
	await pending
	await state.client.refetchQueries({ queryKey: listKey })
	await event
	assert.equal(state.client.getQueryData(['instances', 'summary', 'one']).name, 'Updated instance')
})

test('real instance read errors are still reported', async (t) => {
	const state = fixture(t, async () => {
		throw new Error('Database read failed')
	})
	await assert.rejects(state.emit({ instance_id: 'one', event: 'edited' }), /Database read failed/)
})

test('the event boundary ignores cancelled work but reports real handler failures', async (t) => {
	const source = readFileSync(
		new URL('../src/providers/setup/app-events.ts', import.meta.url),
		'utf8',
	)
	const code = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
	}).outputText
	const exports = {}
	const imports = {
		vue,
		'@tanstack/vue-query': { isCancelledError },
		'@tauri-apps/api/core': {
			Channel: class {
				constructor(handler) {
					this.emit = handler
				}
			},
		},
		'@/providers/app-events': { provideAppEvents: () => {} },
		'@/providers/setup/app-event-codec': { decodeAppEvent: (value) => value },
	}
	new Function('require', 'exports', code)((name) => {
		assert.ok(name in imports, name)
		return imports[name]
	}, exports)
	const scope = vue.effectScope()
	t.after(() => scope.stop())
	const state = scope.run(exports.setupAppEventsProvider)
	const errors = []
	t.mock.method(console, 'error', (...args) => errors.push(args))
	state.events.on('instance', async () => {
		throw new CancelledError({ silent: true })
	})
	const failure = new Error('Native read failed')
	state.events.on('instance', async () => {
		throw failure
	})
	state.channel.emit({ type: 'instance', payload: { instance_id: 'one', event: 'edited' } })
	await new Promise((resolve) => setImmediate(resolve))
	assert.equal(errors.length, 1)
	assert.equal(errors[0][1], failure)
})
