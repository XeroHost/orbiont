import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'

import ts from 'typescript'
import * as vue from 'vue'
import { compileScript, parse } from 'vue/compiler-sfc'

const appRoot = new URL('../src/', import.meta.url)
const uiRoot = new URL('../../../packages/ui/src/', import.meta.url)

async function loadModule(url, require = () => ({}), component = false) {
	const source = await readFile(url, 'utf8')
	const script = component
		? compileScript(parse(source).descriptor, { id: 'performance' }).content
		: source
	const javascript = ts.transpileModule(script, {
		compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
	}).outputText
	const module = { exports: {} }
	new Function('require', 'module', 'exports', javascript)(require, module, module.exports)
	return module.exports
}

const changes = await loadModule(new URL('helpers/account-changes.ts', appRoot))
const lazy = await loadModule(new URL('utils/lazy-load.ts', uiRoot))
const tick = async () => {
	for (let i = 0; i < 8; i++) await vue.nextTick()
}
const deferred = () => {
	let resolve, reject
	const promise = new Promise((yes, no) => {
		resolve = yes
		reject = no
	})
	return { promise, resolve, reject }
}

test('auth notifies only after successful mutations and unsubscribe removes listeners', async () => {
	let notices = 0
	let fail = false
	const native = deferred()
	const auth = await loadModule(new URL('helpers/auth.js', appRoot), (id) =>
		id.includes('account-changes')
			? changes
			: {
					invoke: (command) =>
						fail
							? Promise.reject(Error('native failure'))
							: command.endsWith('login')
								? native.promise
								: Promise.resolve('id'),
				},
	)
	const stop = changes.onAccountChange(() => notices++)
	try {
		const login = auth.login()
		assert.equal(notices, 0)
		native.resolve({ profile: { id: 'account' } })
		await login
		await auth.set_default_user('other')
		await auth.remove_user('account')
		assert.equal(notices, 3)
		await auth.get_default_user()
		await auth.users()
		assert.equal(notices, 3)
		fail = true
		await assert.rejects(auth.set_default_user('failed'), /native failure/)
		assert.equal(notices, 3)
		stop()
		fail = false
		await auth.set_default_user('later')
		assert.equal(notices, 3)
	} finally {
		stop()
	}
})

test('account refresh serializes rapid changes, rejects stale results, and cancels on dispose', async () => {
	const reads = [],
		committed = []
	let concurrent = 0,
		maximum = 0
	const refresh = changes.createAccountRefresh(async (isCurrent) => {
		const read = deferred()
		reads.push(read)
		maximum = Math.max(maximum, ++concurrent)
		const value = await read.promise
		if (isCurrent()) committed.push(value)
		concurrent--
	})
	const first = refresh.request()
	await tick()
	const second = refresh.request()
	const third = refresh.request()
	assert.equal(first, second)
	assert.equal(second, third)
	assert.equal(reads.length, 1)
	reads[0].resolve('stale')
	await tick()
	assert.equal(reads.length, 2)
	reads[1].resolve('latest')
	await first
	assert.deepEqual(committed, ['latest'])
	assert.equal(maximum, 1)
	const pending = refresh.request()
	await tick()
	refresh.dispose()
	reads[2].resolve('unmounted')
	await pending
	await refresh.request()
	assert.equal(reads.length, 3)
	assert.deepEqual(committed, ['latest'])
})

test('Ace loads only on demand, shares concurrent loads and imports the requested mode', async () => {
	const imports = []
	const editor = {}
	let fail = true
	const ace = await loadModule(new URL('utils/ace-loader.ts', uiRoot), (id) => {
		if (id === './lazy-load') return lazy
		imports.push(id)
		if (id === 'vue3-ace-editor') return { VAceEditor: editor }
		if (id.endsWith('mode-json') && fail) throw Error('chunk missing')
		return {}
	})
	assert.deepEqual(imports, [])
	const first = ace.loadAceEditor('json')
	const second = ace.loadAceEditor('json')
	await assert.rejects(first, /chunk missing/)
	await assert.rejects(second, /chunk missing/)
	assert.equal(imports.filter((id) => id === 'vue3-ace-editor').length, 1)
	assert.equal(imports.filter((id) => id.endsWith('mode-json')).length, 1)
	fail = false
	assert.equal(await ace.loadAceEditor('json'), editor)
	await ace.loadAceEditor('mcfunction')
	await ace.loadAceEditor('mclog')
	await ace.loadAceEditor('unrecognized')
	assert.equal(imports.filter((id) => id.endsWith('mode-json')).length, 2)
	assert.equal(imports.includes('./ace-mode-mcfunction'), true)
	assert.equal(imports.includes('./ace-mode-log'), true)
	assert.equal(imports.includes('ace-builds/src-noconflict/mode-text'), true)
	assert.equal(
		imports.some((id) => id.endsWith('mode-java')),
		false,
	)
})

test('actual file editor skips Ace for images and retries failed text loads without losing content', async () => {
	let loads = 0,
		fail = true,
		unmount,
		future
	const oldWindow = globalThis.window
	globalThis.window = { scrollTo() {}, removeEventListener() {} }
	const scope = vue.effectScope()
	try {
		const component = await loadModule(
			new URL('layouts/shared/files-tab/components/editor/FileEditor.vue', uiRoot),
			(id) => {
				if (id === 'vue')
					return {
						...vue,
						onMounted() {},
						onUnmounted: (callback) => {
							unmount = callback
						},
					}
				if (id.includes('ace-loader'))
					return {
						loadAceEditor: async () => {
							loads++
							if (future) return future.promise
							if (fail) throw Error('failed chunk')
							return {}
						},
					}
				if (id.includes('file-manager'))
					return {
						injectFileManager: () => ({
							readFile: async () => 'original',
							readFileAsBlob: async () => new Blob(['image']),
						}),
					}
				if (id.includes('i18n'))
					return {
						defineMessages: (value) => value,
						useVIntl: () => ({ formatMessage: (message) => message.defaultMessage }),
					}
				if (id.includes('web-notifications'))
					return { injectNotificationManager: () => ({ addNotification() {} }) }
				if (id.includes('file-extensions'))
					return {
						getFileExtension: (name) => name.split('.').at(-1),
						getEditorLanguage: () => 'json',
						isImageFile: (ext) => ext === 'png',
					}
				if (id === '#ui/providers') return { injectApiClient: () => ({}) }
				return {}
			},
			true,
		)
		const props = vue.reactive({ file: { name: 'picture.png', path: '/picture.png' } })
		const state = scope.run(() =>
			component.default.setup(props, {
				expose() {},
				emit() {
					throw Error('unexpected close')
				},
			}),
		)
		await tick()
		assert.equal(loads, 0)
		assert.equal(state.isEditingImage.value, true)
		props.file = { name: 'config.json', path: '/config.json' }
		await tick()
		assert.equal(loads, 1)
		assert.equal(state.editorLoadFailed.value, true)
		assert.equal(state.fileContent.value, 'original')
		state.fileContent.value = 'draft'
		fail = false
		await state.retryEditor()
		assert.equal(loads, 2)
		assert.equal(state.editorLoadFailed.value, false)
		assert.equal(state.fileContent.value, 'draft')
		assert.equal(state.hasUnsavedChanges.value, true)
		future = deferred()
		props.file = { name: 'other.json', path: '/other.json' }
		await tick()
		assert.equal(state.isEditorLoading.value, true)
		unmount()
		future.resolve({})
		await tick()
		assert.equal(state.editorComponent.value, null)
		assert.equal(state.isEditorLoading.value, false)
	} finally {
		scope.stop()
		if (oldWindow === undefined) delete globalThis.window
		else globalThis.window = oldWindow
	}
})

test('actual history modal loads Ace only when opened and permits retry after a failed chunk', async () => {
	let loads = 0,
		shows = 0,
		errors = 0,
		fail = true
	const component = await loadModule(
		new URL(
			'components/ui/settings/instances/instances-synced-settings/command-history-modal.vue',
			appRoot,
		),
		(id) => {
			if (id === 'vue') return vue
			if (id.includes('ace-loader'))
				return {
					loadAceEditor: async (language) => {
						assert.equal(language, 'mcfunction')
						loads++
						if (fail) throw Error('failed chunk')
						return {}
					},
				}
			if (id === '@orbiont/ui')
				return {
					defineMessages: (value) => value,
					useVIntl: () => ({ formatMessage() {} }),
					injectNotificationManager: () => ({ handleError: () => errors++ }),
				}
			if (id.includes('vue-query'))
				return {
					useQuery: () => ({ refetch: async () => ({ isSuccess: true, data: 'say hello' }) }),
					useMutation: () => ({}),
					useQueryClient: () => ({}),
				}
			if (id.includes('synced-options')) return { commandHistoryQueryOptions: () => ({}) }
			return {}
		},
		true,
	)
	const scope = vue.effectScope()
	try {
		const state = scope.run(() => component.default.setup({}, { expose() {} }))
		state.modal.value = { show: () => shows++ }
		assert.equal(loads, 0)
		await state.show()
		assert.equal(loads, 1)
		assert.equal(shows, 0)
		assert.equal(errors, 1)
		fail = false
		await state.show()
		assert.equal(loads, 2)
		assert.equal(shows, 1)
		assert.equal(state.commandHistory.value, 'say hello')
	} finally {
		scope.stop()
	}
})

test('scroll coalesces events, corrects sibling shifts and borders, and cancels listeners/frames', async () => {
	const originals = Object.fromEntries(
		['window', 'Window', 'getComputedStyle', 'ResizeObserver', 'MutationObserver'].map((key) => [
			key,
			globalThis[key],
		]),
	)
	const frames = new Map(),
		observers = [],
		mutations = []
	let frameId = 0,
		reads = 0,
		scrollCalls = 0,
		resizeCalls = 0
	class FakeWindow extends EventTarget {
		scrollY = 0
		innerHeight = 800
		requestAnimationFrame(callback) {
			frames.set(++frameId, callback)
			return frameId
		}
		cancelAnimationFrame(id) {
			frames.delete(id)
		}
	}
	class Element extends EventTarget {
		parentElement = null
		children = []
		scrollTop = 0
		clientHeight = 400
		clientTop = 2
		top = 100
		overflowY = 'visible'
		getBoundingClientRect() {
			reads++
			return { top: this.top, bottom: this.top + this.clientHeight }
		}
	}
	globalThis.Window = FakeWindow
	globalThis.window = vue.markRaw(new FakeWindow())
	globalThis.getComputedStyle = (element) => ({ overflowY: element.overflowY })
	globalThis.ResizeObserver = class {
		observed = new Set()
		constructor(callback) {
			this.callback = callback
			observers.push(this)
		}
		observe(element) {
			this.observed.add(element)
		}
		disconnect() {
			this.observed.clear()
		}
	}
	globalThis.MutationObserver = class {
		observed = new Map()
		constructor(callback) {
			this.callback = callback
			mutations.push(this)
		}
		observe(element, options) {
			this.observed.set(element, options)
		}
		disconnect() {
			this.observed.clear()
		}
	}
	const scope = vue.effectScope()
	try {
		const { useScrollViewport } = await loadModule(
			new URL('composables/virtual-scroll.ts', uiRoot),
			() => vue,
		)
		const parent = vue.markRaw(new Element()),
			list = vue.markRaw(new Element()),
			header = vue.markRaw(new Element())
		parent.overflowY = 'auto'
		list.parentElement = parent
		parent.children = [header, list]
		list.top = 182
		const state = scope.run(() =>
			useScrollViewport({ onScroll: () => scrollCalls++, onResize: () => resizeCalls++ }),
		)
		state.listContainer.value = list
		await tick()
		assert.equal(state.containerOffset.value, 80)
		assert.equal(observers[0].observed.has(header), true)
		reads = 0
		parent.scrollTop = 300
		list.top = -118
		for (let i = 0; i < 100; i++) parent.dispatchEvent(new Event('scroll'))
		assert.equal(frames.size, 1)
		assert.equal(reads, 0)
		const flushFrame = () => {
			const callbacks = [...frames.values()]
			frames.clear()
			callbacks.forEach((callback) => callback())
		}
		flushFrame()
		assert.equal(scrollCalls, 1)
		assert.equal(reads, 2)
		assert.equal(state.relativeScrollTop.value, 220)
		list.top += 40
		observers[0].callback()
		window.dispatchEvent(new Event('resize'))
		flushFrame()
		assert.equal(resizeCalls, 1)
		assert.equal(state.containerOffset.value, 120)
		assert.equal(state.relativeScrollTop.value, 180)
		assert.equal(mutations[0].observed.get(list).childList, false)
		const banner = vue.markRaw(new Element())
		parent.children.push(banner)
		list.top += 20
		mutations[0].callback()
		flushFrame()
		assert.equal(state.containerOffset.value, 140)
		assert.equal(observers[0].observed.has(banner), true)
		parent.children = [list]
		mutations[0].callback()
		flushFrame()
		assert.equal(observers[0].observed.has(banner), false)
		parent.dispatchEvent(new Event('scroll'))
		assert.equal(frames.size, 1)
		scope.stop()
		assert.equal(frames.size, 0)
		assert.equal(observers[0].observed.size, 0)
		assert.equal(mutations[0].observed.size, 0)
		parent.dispatchEvent(new Event('scroll'))
		window.dispatchEvent(new Event('resize'))
		observers[0].callback()
		mutations[0].callback()
		assert.equal(frames.size, 0)
		assert.equal(mutations[0].observed.size, 0)
		assert.equal(scrollCalls, 1)
	} finally {
		scope.stop()
		for (const [key, value] of Object.entries(originals)) {
			if (value === undefined) delete globalThis[key]
			else globalThis[key] = value
		}
	}
})
