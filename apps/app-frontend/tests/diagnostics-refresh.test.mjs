import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'

import { QueryClient, useQuery, useQueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import ts from 'typescript'
import * as vue from 'vue'
import { compileScript, compileTemplate, parse } from 'vue/compiler-sfc'

const page = parse(
	await readFile(new URL('../src/pages/Bedrock.vue', import.meta.url), 'utf8'),
).descriptor
const diagnostic = parse(
	await readFile(
		new URL('../src/components/ui/bedrock/BedrockDiagnostics.vue', import.meta.url),
		'utf8',
	),
).descriptor
const javascript = ts.transpileModule(compileScript(page, { id: 'diagnostics-refresh' }).content, {
	compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText
const ready = {
	supported: true,
	game_running: false,
	game: { version: '1.21', can_launch: true },
	launcher: { can_launch: true },
	launch: { state: 'idle', elapsed_ms: 0 },
}
function deferred() {
	let resolve, reject
	const promise = new Promise((yes, no) => {
		resolve = yes
		reject = no
	})
	return { promise, resolve, reject }
}
async function flush() {
	await new Promise(setImmediate)
	await vue.nextTick()
}
async function fixture(t, initial = true) {
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false, gcTime: Infinity } },
	})
	if (initial) {
		client.setQueryData(['bedrock', 'status'], ready)
		client.setQueryData(['bedrock', 'process'], {
			game_running: false,
			launch: ready.launch,
		})
	}
	const statusCalls = [],
		processCalls = []
	const messages = new Proxy({}, { get: (_, key) => ({ defaultMessage: String(key) }) })
	const imports = {
		vue,
		'@tanstack/vue-query': { useQuery, useQueryClient },
		'vue-router': { useRouter: () => ({}) },
		'@/providers/breadcrumbs': { useRootBreadcrumb() {} },
		'@orbiont/assets': {},
		'@orbiont/ui': {
			useVIntl: () => ({ formatMessage: (message) => message.defaultMessage }),
			commonMessages: messages,
		},
		'@/helpers/bedrock-messages': { bedrockMessages: messages },
		'@/helpers/bedrock-catalog-messages': { bedrockCatalogMessages: messages },
		'@/helpers/bedrock': {
			bedrockStatusQueryOptions: () => ({
				queryKey: ['bedrock', 'status'],
				queryFn: () => imports['@/helpers/bedrock'].getBedrockStatus(false),
				staleTime: Infinity,
				retry: false,
			}),
			getBedrockStatus: (refresh) => {
				const request = deferred()
				statusCalls.push({ ...request, refresh })
				return request.promise
			},
			getBedrockProcessStatus: () => {
				const request = deferred()
				processCalls.push(request)
				return request.promise
			},
			isBedrockLaunchPending: (launch) => ['requested', 'starting'].includes(launch?.state),
		},
	}
	const module = { exports: {} }
	new Function('require', 'module', 'exports', javascript)(
		(id) => {
			if (id.endsWith('.vue')) return {}
			if (!(id in imports)) throw Error(`Unexpected import ${id}`)
			return imports[id]
		},
		module,
		module.exports,
	)
	let state
	const renderer = vue.createRenderer({
		createComment: () => ({}),
		insert() {},
		remove() {},
		parentNode: () => null,
		nextSibling: () => null,
	})
	const component = {
		...module.exports.default,
		setup(props, context) {
			state = module.exports.default.setup(props, context)
			return () => null
		},
	}
	const app = renderer.createApp(component)
	app.use(VueQueryPlugin, { queryClient: client })
	app.mount({})
	t.after(() => {
		app.unmount()
		client.clear()
	})
	await flush()
	// The process query refetches on mount because its snapshot is immediately stale.
	if (initial && processCalls.length) {
		processCalls[0].resolve({ game_running: false, launch: ready.launch })
		await flush()
		processCalls.length = 0
	}
	return { state, statusCalls, processCalls, client }
}

test('automatic process and installation refreshes retain data and never show manual loading', async (t) => {
	const { state, statusCalls, processCalls } = await fixture(t)
	const process = state.processStatus.refetch()
	const status = state.status.refetch()
	assert.equal(state.refreshing.value, false)
	assert.equal(state.canPlay.value, true)
	assert.equal(state.diagnosticStatus.value.launch.state, 'idle')
	processCalls[0].resolve({ game_running: true, launch: { state: 'running', elapsed_ms: 2000 } })
	statusCalls[0].resolve({ ...ready, game: { ...ready.game, version: '1.22' } })
	await Promise.all([process, status])
	await flush()
	assert.equal(state.running.value, true)
	assert.equal(state.diagnosticStatus.value.launch.state, 'running')
	assert.equal(state.installation.value.game.version, '1.22')
	assert.equal(state.refreshing.value, false)
})

test('manual verification joins an automatic request and rejects repeated clicks until both reads settle', async (t) => {
	const { state, statusCalls, processCalls } = await fixture(t)
	const automatic = state.processStatus.refetch()
	const manual = state.refreshStatus()
	assert.equal(state.refreshing.value, true)
	assert.equal(state.busy.value, false)
	assert.equal(state.canPlay.value, true)
	await state.refreshStatus()
	assert.equal(processCalls.length, 1)
	assert.equal(statusCalls.length, 1)
	assert.equal(statusCalls[0].refresh, true)
	statusCalls[0].resolve(ready)
	await flush()
	assert.equal(state.refreshing.value, true)
	processCalls[0].resolve({ game_running: true, launch: { state: 'starting', elapsed_ms: 3000 } })
	await Promise.all([automatic, manual])
	await flush()
	assert.equal(state.refreshing.value, false)
	assert.equal(state.running.value, true)
	assert.equal(state.launchPending.value, true)
})

test('failed manual verification retains the valid card, exposes failures and releases its indicator', async (t) => {
	const { state, statusCalls, processCalls } = await fixture(t)
	const manual = state.refreshStatus()
	statusCalls[0].reject(Error('installation unavailable'))
	processCalls[0].reject(Error('process unavailable'))
	await manual
	await flush()
	assert.equal(state.refreshing.value, false)
	assert.equal(state.canPlay.value, true)
	assert.equal(state.diagnosticStatus.value.game.version, '1.21')
	assert.equal(state.status.isError.value, true)
	assert.equal(state.processStatus.isError.value, true)
})

test('initial detection failure has no fabricated data and remains visible to the page', async (t) => {
	const { state, statusCalls } = await fixture(t, false)
	assert.equal(state.status.isPending.value, true)
	statusCalls[0].reject(Error('no installation snapshot'))
	await flush()
	assert.equal(state.status.isPending.value, false)
	assert.equal(state.status.isError.value, true)
	assert.equal(state.diagnosticStatus.value, null)
	assert.equal(state.refreshing.value, false)
})

test('diagnostics renders a stable verification icon and label while only manual loading animates it', () => {
	const compiled = compileTemplate({
		source: diagnostic.template.content,
		filename: 'BedrockDiagnostics.vue',
		id: 'diagnostics-template',
		compilerOptions: { mode: 'function' },
	})
	assert.deepEqual(compiled.errors, [])
	const render = new Function('Vue', compiled.code)({
		...vue,
		resolveComponent: (name) => ({ name }),
	})
	const messages = new Proxy({}, { get: (_, key) => key })
	function card(refreshing, failed = false) {
		return render(
			{
				status: ready,
				busy: false,
				refreshing,
				failed,
				messages,
				bedrockMessages: messages,
				formatMessage: String,
				emit() {},
			},
			[],
		)
	}
	const originalWarning = console.warn
	console.warn = () => {}
	try {
		const resting = card(false),
			manual = card(true)
		assert.equal(resting.children.length, manual.children.length)
		const actions = resting.children.at(-1).children
		const refreshingActions = manual.children.at(-1).children
		assert.equal(actions[2].props.loading, false)
		assert.equal(refreshingActions[2].props.loading, true)
		const icon = actions[2].children.default()[0]
		const spinningIcon = refreshingActions[2].children.default()[0]
		assert.equal(icon.props.name, 'refresh')
		assert.equal(spinningIcon.props.name, 'refresh')
		assert.equal(icon.props.class, '')
		assert.equal(spinningIcon.props.class, 'motion-safe:animate-spin')
		assert.equal(actions[3].props.disabled, undefined)
		assert.equal(refreshingActions[3].props.disabled, undefined)
		assert.equal(resting.children[1].type, vue.Comment)
		assert.notEqual(card(false, true).children[1].type, vue.Comment)
	} finally {
		console.warn = originalWarning
	}
})
