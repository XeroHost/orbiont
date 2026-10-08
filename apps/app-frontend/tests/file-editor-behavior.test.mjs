import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { test } from 'node:test'

import ts from 'typescript'
import * as vue from 'vue'
import { compileScript, parse } from 'vue/compiler-sfc'

const source = await readFile(
	new URL(
		'../../../packages/ui/src/layouts/shared/files-tab/components/editor/FileEditor.vue',
		import.meta.url,
	),
	'utf8',
)
const script = compileScript(parse(source).descriptor, { id: 'editor-behavior' }).content
const javascript = ts.transpileModule(script, {
	compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText

function editor(read, write) {
	const notices = [],
		emitted = []
	const ctx = { readFile: read, writeFile: write }
	const require = (id) => {
		if (id === 'vue') return { ...vue, onMounted() {}, onUnmounted() {} }
		if (id.includes('ace-loader')) return { loadAceEditor: async () => ({}) }
		if (id.includes('file-manager')) return { injectFileManager: () => ctx }
		if (id.includes('i18n'))
			return {
				defineMessages: (value) => value,
				useVIntl: () => ({ formatMessage: (message) => message.defaultMessage }),
			}
		if (id.includes('web-notifications'))
			return {
				injectNotificationManager: () => ({ addNotification: (message) => notices.push(message) }),
			}
		if (id.includes('file-extensions'))
			return {
				getFileExtension: (name) => name.split('.').at(-1),
				getEditorLanguage: () => 'text',
				isImageFile: () => false,
			}
		if (id === '#ui/providers') return { injectApiClient: () => ({}) }
		return {}
	}
	const module = { exports: {} }
	new Function('require', 'module', 'exports', javascript)(require, module, module.exports)
	const props = vue.reactive({
		file: { name: 'options.txt', path: '/options.txt' },
		editorComponent: null,
	})
	let exposed
	const scope = vue.effectScope()
	const previousWindow = globalThis.window
	globalThis.window = { scrollTo() {} }
	scope.run(() =>
		module.exports.default.setup(props, {
			expose: (value) => {
				exposed = value
			},
			emit: (value) => emitted.push(value),
		}),
	)
	return {
		exposed,
		props,
		notices,
		emitted,
		dispose() {
			scope.stop()
			if (previousWindow === undefined) delete globalThis.window
			else globalThis.window = previousWindow
		},
	}
}
const flush = async () => {
	await vue.nextTick()
	await Promise.resolve()
	await vue.nextTick()
}

test('actual unsaved modal shares pending prompts and treats dismissal/unmount as cancel', async () => {
	const source = await readFile(
		new URL(
			'../../../packages/ui/src/layouts/shared/files-tab/components/modals/FileUnsavedChangesModal.vue',
			import.meta.url,
		),
		'utf8',
	)
	const script = compileScript(parse(source).descriptor, { id: 'unsaved-modal-test' }).content
	const javascript = ts.transpileModule(script, {
		compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
	}).outputText
	let dispose
	const require = (id) => {
		if (id === 'vue')
			return {
				...vue,
				onUnmounted: (callback) => {
					dispose = callback
				},
			}
		if (id.includes('i18n'))
			return {
				defineMessages: (value) => value,
				useVIntl: () => ({ formatMessage: (message) => message.defaultMessage }),
			}
		return {}
	}
	const module = { exports: {} }
	new Function('require', 'module', 'exports', javascript)(require, module, module.exports)
	const state = module.exports.default.setup({}, { expose() {} })
	let shows = 0
	state.modal.value = {
		show() {
			shows++
		},
		hide() {},
	}
	const first = state.prompt(),
		second = state.prompt()
	assert.equal(first, second)
	assert.equal(shows, 1)
	state.settle('cancel')
	assert.deepEqual(await Promise.all([first, second]), ['cancel', 'cancel'])
	const third = state.prompt()
	dispose()
	assert.equal(await third, 'cancel')
})

test('actual editor keeps draft and stays open after failed or conflicting saves', async () => {
	for (const reason of ['save failed', 'Document changed externally']) {
		const view = editor(
			async () => 'original',
			async () => {
				throw Error(reason)
			},
		)
		try {
			await flush()
			view.exposed.fileContent.value = 'draft'
			await view.exposed.saveFileContent(true)
			assert.equal(view.exposed.fileContent.value, 'draft')
			assert.equal(view.exposed.hasUnsavedChanges.value, true)
			assert.deepEqual(view.emitted, [])
			assert.equal(view.notices.at(-1).type, 'error')
		} finally {
			view.dispose()
		}
	}
})
test('actual editor retains edits typed during a save and prevents duplicate writes', async () => {
	let finish,
		writes = 0
	const view = editor(
		async () => 'original',
		async () => {
			writes++
			await new Promise((resolve) => {
				finish = resolve
			})
		},
	)
	try {
		await flush()
		view.exposed.fileContent.value = 'first draft'
		const saving = view.exposed.saveFileContent(true)
		view.exposed.fileContent.value = 'new draft'
		await view.exposed.saveFileContent(true)
		assert.equal(writes, 1)
		finish()
		await saving
		assert.equal(view.exposed.fileContent.value, 'new draft')
		assert.equal(view.exposed.hasUnsavedChanges.value, true)
		assert.deepEqual(view.emitted, [])
	} finally {
		view.dispose()
	}
})
test('a stale file read cannot overwrite the next document', async () => {
	let finish
	const view = editor(
		async (path) =>
			path === '/options.txt'
				? new Promise((resolve) => {
						finish = resolve
					})
				: 'new document',
		async () => {},
	)
	try {
		view.props.file = { name: 'config.txt', path: '/config.txt' }
		await flush()
		finish('stale document')
		await flush()
		assert.equal(view.exposed.fileContent.value, 'new document')
	} finally {
		view.dispose()
	}
})

test('bounded Ace modes load offline, disable workers, and preserve mcfunction plus search/replace', async () => {
	const require = createRequire(new URL('../../../packages/ui/package.json', import.meta.url))
	const ace = require('ace-builds')
	const previousAce = globalThis.ace
	globalThis.ace = ace
	try {
		for (const filename of ['ace-theme.ts', 'ace-mode-log.ts', 'ace-mode-mcfunction.ts']) {
			const text = await readFile(
				new URL(`../../../packages/ui/src/utils/${filename}`, import.meta.url),
				'utf8',
			)
			assert.equal(text.includes('esm-resolver'), false)
			const output = ts.transpileModule(text, {
				compilerOptions: {
					target: ts.ScriptTarget.ES2022,
					module: ts.ModuleKind.CommonJS,
					esModuleInterop: true,
				},
			}).outputText
			new Function('require', 'exports', output)(
				(id) =>
					id === './ace-define'
						? { defineAceModule: ace.define }
						: id.includes('?raw')
							? ''
							: require(id),
				{},
			)
		}
		const evaluateUtility = async (filename, resolveImport) => {
			const text = await readFile(
				new URL(`../../../packages/ui/src/utils/${filename}`, import.meta.url),
				'utf8',
			)
			const output = ts.transpileModule(text, {
				compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
			}).outputText
			const exports = {}
			new Function('require', 'exports', output)(resolveImport, exports)
			return exports
		}
		const lazy = await evaluateUtility('lazy-load.ts', require)
		const loader = await evaluateUtility('ace-loader.ts', (id) => {
			if (id === './lazy-load') return lazy
			if (id === './ace-theme' || id === './ace-mode-log' || id === './ace-mode-mcfunction')
				return {}
			if (id === 'vue3-ace-editor') return { VAceEditor: {} }
			return require(id)
		})
		assert.equal(ace.require('ace/mode/json'), undefined)
		await loader.loadAceEditor('json')
		await loader.loadAceEditor('javascript')
		const session = new ace.EditSession('say hello\nsay hello')
		session.setMode('ace/mode/mcfunction')
		assert.equal(session.getMode().$id, 'ace/mode/mcfunction')
		assert.equal(session.getUseWorker(), false)
		const Search = ace.require('ace/search').Search
		const search = new Search().set({ needle: 'hello', wrap: true })
		assert.equal(search.findAll(session).length, 2)
		assert.equal(search.replace('hello', 'world'), 'world')
		assert.ok(ace.require('ace/mode/json').Mode)
		assert.ok(ace.require('ace/mode/javascript').Mode)
	} finally {
		if (previousAce === undefined) delete globalThis.ace
		else globalThis.ace = previousAce
	}
})
