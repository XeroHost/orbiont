import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'

import ts from 'typescript'
import * as vue from 'vue'
import { compileScript, compileTemplate, parse } from 'vue/compiler-sfc'

import * as retention from '../src/helpers/recovery-retention.ts'

const source = await readFile(
	new URL('../src/components/ui/management/LocalManagementCenter.vue', import.meta.url),
	'utf8',
)
const descriptor = parse(source).descriptor
const script = compileScript(descriptor, { id: 'local-management-test' }).content
const javascript = ts.transpileModule(script, {
	compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText
const copy = {
	id: 'copy',
	rootId: 'root',
	path: 'world',
	created: 10,
	size: 99,
	limited: false,
	removable: true,
	state: 'applied',
}
function center(adapter) {
	const scope = vue.effectScope()
	const module = { exports: {} }
	const messages = new Proxy({}, { get: (_, key) => ({ defaultMessage: String(key) }) })
	const require = (id) => {
		if (id === 'vue') return vue
		if (id.includes('management-messages')) return { managementMessages: messages }
		if (id.includes('recovery-retention')) return retention
		return {
			useFormatBytes: () => String,
			useVIntl: () => ({
				locale: vue.ref('en'),
				formatMessage: (message) => message.defaultMessage,
			}),
			commonMessages: messages,
		}
	}
	new Function('require', 'module', 'exports', javascript)(require, module, module.exports)
	let state
	scope.run(() => {
		state = module.exports.default.setup(vue.reactive({ adapter, disabled: false }), {
			expose() {},
		})
	})
	state.modal.value = { show() {}, hide() {} }
	return { state, dispose: () => scope.stop() }
}

test('common recovery view preserves loading/error state and requires a restorable preview before restore', async () => {
	let restoreCalls = 0,
		loadFinish
	const adapter = {
		edition: 'bedrock',
		list: async () =>
			new Promise((resolve) => {
				loadFinish = resolve
			}),
		preview: async () => ({ copy, canRestore: false, conflicts: ['destination changed'] }),
		restore: async () => {
			restoreCalls++
		},
	}
	const view = center(adapter)
	try {
		const loading = view.state.show('recovery')
		assert.equal(view.state.busy.value, true)
		loadFinish({ entries: [copy], total: 1, incomplete: true })
		await loading
		assert.equal(view.state.incomplete.value, true)
		assert.equal(view.state.copies.value[0].size, 99)
		await view.state.restore()
		assert.equal(restoreCalls, 0)
		await view.state.selectCopy(copy)
		await view.state.restore()
		assert.equal(restoreCalls, 0)
		assert.deepEqual([...view.state.preview.value.conflicts], ['destination changed'])
		adapter.preview = async () => {
			throw Error('stale recovery')
		}
		await view.state.selectCopy(copy)
		assert.equal(view.state.failed.value, true)
		assert.equal(view.state.preview.value, null)
	} finally {
		view.dispose()
	}
})
test('common storage view removes only explicit selection after second confirmation and retains failed selection', async () => {
	const selected = { ...copy, category: 'recovery' },
		protectedData = { ...copy, id: 'world', category: 'content', removable: false }
	const removed = []
	let fail = true
	const view = center({
		edition: 'java',
		storage: async () => ({
			categories: { data: 100, recovery: 99 },
			entries: [selected, protectedData],
			incomplete: false,
		}),
		remove: async (entries) => {
			removed.push(entries.map((entry) => entry.id))
			if (fail) throw Error('locked')
		},
	})
	try {
		await view.state.show('storage')
		view.state.toggle(protectedData)
		assert.equal(view.state.selected.value.length, 0)
		view.state.toggle(selected)
		await view.state.removeSelected()
		assert.equal(removed.length, 0)
		view.state.confirmRemoval.value = true
		await view.state.removeSelected()
		assert.deepEqual(removed, [['copy']])
		assert.equal(view.state.failed.value, true)
		assert.equal(view.state.selected.value.length, 1)
		fail = false
		await view.state.removeSelected()
		assert.equal(view.state.selection.value.size, 0)
	} finally {
		view.dispose()
	}
})
test('new common recovery template compiles and renders preview text through interpolation', () => {
	const result = compileTemplate({
		source: descriptor.template.content,
		filename: 'LocalManagementCenter.vue',
		id: 'management-template',
	})
	assert.deepEqual(result.errors, [])
	assert.equal(result.code.includes('toDisplayString(_ctx.preview.content)'), true)
})

test('storage rows use the shared accessible checkbox and toggle once, preserving protected entries', async (t) => {
	const checkboxSource = await readFile(
		new URL('../../../packages/ui/src/components/base/Checkbox.vue', import.meta.url),
		'utf8',
	)
	const evaluate = (componentDescriptor, require) => {
		const code = ts.transpileModule(
			compileScript(componentDescriptor, { id: 'management-ui-test', inlineTemplate: true })
				.content,
			{ compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } },
		).outputText
		const module = { exports: {} }
		new Function('require', 'module', 'exports', code)(require, module, module.exports)
		return module.exports.default
	}
	const icon = { render: () => vue.h('svg') }
	const Checkbox = evaluate(parse(checkboxSource).descriptor, (id) =>
		id === 'vue' ? vue : { CheckIcon: icon, MinusIcon: icon },
	)
	const passthrough = {
		setup:
			(_, { slots }) =>
			() =>
				vue.h('div', slots.default?.()),
	}
	const Modal = {
		setup(_, { slots, expose }) {
			expose({ show() {}, hide() {} })
			return () => vue.h('div', slots.default?.())
		},
	}
	const messages = new Proxy({}, { get: (_, key) => ({ defaultMessage: String(key) }) })
	const component = evaluate(descriptor, (id) => {
		if (id === 'vue') return { ...vue, vModelText: {} }
		if (id.includes('management-messages')) return { managementMessages: messages }
		if (id.includes('recovery-retention')) return retention
		return {
			Checkbox,
			Button: passthrough,
			Admonition: passthrough,
			NewModal: Modal,
			useFormatBytes: () => String,
			useVIntl: () => ({
				locale: vue.ref('en'),
				formatMessage: (message) => message.defaultMessage,
			}),
			commonMessages: messages,
		}
	})
	const node = (type, text = '') => ({ type, text, props: {}, children: [], parent: null })
	const renderer = vue.createRenderer({
		createElement: node,
		createText: (text) => node('text', text),
		createComment: (text) => node('comment', text),
		setText: (element, text) => (element.text = text),
		setElementText: (element, text) => (element.text = text),
		patchProp: (element, key, _previous, next) => (element.props[key] = next),
		insert(element, parent, anchor) {
			if (element.parent)
				element.parent.children.splice(element.parent.children.indexOf(element), 1)
			element.parent = parent
			const index = anchor ? parent.children.indexOf(anchor) : -1
			if (index === -1) parent.children.push(element)
			else parent.children.splice(index, 0, element)
		},
		remove(element) {
			if (element.parent)
				element.parent.children.splice(element.parent.children.indexOf(element), 1)
		},
		parentNode: (element) => element.parent,
		nextSibling: (element) =>
			element.parent?.children[element.parent.children.indexOf(element) + 1],
	})
	const root = node('root')
	const protectedEntry = { ...copy, id: 'protected', category: 'content', removable: false }
	const eligible = { ...copy, category: 'recovery' }
	const disabled = vue.ref(false)
	const adapter = {
		edition: 'java',
		storage: async () => ({
			entries: [eligible, protectedEntry],
			categories: {},
			incomplete: false,
		}),
	}
	let instance
	const app = renderer.createApp({
		setup: () => () =>
			vue.h(component, { adapter, disabled: disabled.value, ref: (value) => (instance = value) }),
	})
	app.mount(root)
	t.after(() => app.unmount())
	await instance.show('storage')
	await vue.nextTick()
	const rows = () => {
		const walk = (element) => [element, ...element.children.flatMap(walk)]
		return walk(root).filter((element) => element.props.role === 'checkbox')
	}
	assert.equal(rows().length, 2)
	assert.equal(rows()[0].type, 'button')
	assert.equal(rows()[0].props.type, 'button')
	assert.equal(rows()[0].props['aria-label'], eligible.path)
	assert.equal(rows()[0].props['aria-checked'], false)
	assert.equal(rows()[1].props.disabled, true)
	assert.match(rows()[0].children[0].props.class, /border-solid/)
	rows()[0].props.onClick({})
	await vue.nextTick()
	assert.equal(rows()[0].props['aria-checked'], true)
	rows()[0].props.onClick({})
	await vue.nextTick()
	assert.equal(rows()[0].props['aria-checked'], false)
	rows()[1].props.onClick({})
	await vue.nextTick()
	assert.equal(rows()[1].props['aria-checked'], false)
	disabled.value = true
	await vue.nextTick()
	assert.equal(rows()[0].props.disabled, true)
	rows()[0].props.onClick({})
	await vue.nextTick()
	assert.equal(rows()[0].props['aria-checked'], false)
})

test('retention previews eligible old copies without deleting and refuses incomplete inventory', async () => {
	const old = { ...copy, id: 'old', created: 1, category: 'recovery' }
	const newest = { ...copy, id: 'new', created: Date.now() / 1000, category: 'recovery' }
	let incomplete = false
	let removals = 0
	const view = center({
		edition: 'java',
		list: async () => ({ entries: [newest, old], total: 2, incomplete }),
		storage: async () => ({ entries: [newest, old], categories: {}, incomplete: false }),
		remove: async () => {
			removals++
		},
	})
	try {
		await view.state.show('storage')
		view.state.retentionKeep.value = 1
		await view.state.selectOldCopies()
		assert.deepEqual([...view.state.selection.value], ['root/old'])
		assert.equal(view.state.confirmRemoval.value, false)
		await view.state.removeSelected()
		assert.equal(removals, 0)
		view.state.confirmRemoval.value = true
		incomplete = true
		await view.state.selectOldCopies()
		assert.equal(view.state.failed.value, true)
		assert.equal(view.state.incomplete.value, true)
		assert.equal(view.state.selection.value.size, 0)
		assert.equal(view.state.confirmRemoval.value, false)
		await view.state.removeSelected()
		assert.equal(removals, 0)
	} finally {
		view.dispose()
	}
})
