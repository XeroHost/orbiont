import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { test } from 'node:test'

import { compileScript, parse } from '@vue/compiler-sfc'
import * as vue from 'vue'

const ts = createRequire(import.meta.url)('typescript')
const { descriptor } = parse(
	readFileSync(
		new URL('../src/components/ui/sidebar/SidebarFeaturedModpacks.vue', import.meta.url),
		'utf8',
	),
)
const code = ts.transpileModule(
	compileScript(descriptor, { id: 'featured-carousel-test' }).content,
	{ compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } },
).outputText

function fixture(t, count = 6) {
	t.mock.timers.enable({ apis: ['setTimeout'] })
	const items = vue.ref(
		Array.from({ length: count }, (_, index) => ({
			projectId: `project-${index}`,
			provider: index % 2 ? 'curseforge' : 'modrinth',
			name: `Pack ${index}`,
			summary: 'Summary',
			imageUrl: null,
			iconUrl: null,
		})),
	)
	const visibility = vue.ref('visible')
	const exports = {}
	const imports = {
		vue,
		'@orbiont/assets': {},
		'@orbiont/ui': {
			defineMessages: (value) => value,
			useVIntl: () => ({ formatMessage: (message) => message.defaultMessage }),
			useFormatNumber: () => String,
		},
		'@vueuse/core': { useDocumentVisibility: () => visibility },
		'vue-router': {},
		'@/composables/use-featured-modpacks': {
			useFeaturedModpacks: () => ({ items, loading: vue.ref(false), retry: () => {} }),
		},
	}
	new Function('require', 'exports', code)((name) => {
		if (!(name in imports)) throw new Error(`Unexpected import ${name}`)
		return imports[name]
	}, exports)
	let state
	const component = {
		...exports.default,
		setup(props, context) {
			state = exports.default.setup(props, context)
			return () => null
		},
	}
	const renderer = vue.createRenderer({
		createComment: () => ({}),
		insert: () => {},
		remove: () => {},
		parentNode: () => null,
		nextSibling: () => null,
	})
	const app = renderer.createApp(component)
	app.mount({})
	t.after(() => app.unmount())
	return { state, items, visibility, unmount: () => app.unmount() }
}

test('rotates after eight seconds and wraps the sixth modpack to the first', async (t) => {
	const { state } = fixture(t)
	t.mock.timers.tick(7999)
	await vue.nextTick()
	assert.equal(state.current.value.projectId, 'project-0')
	t.mock.timers.tick(1)
	await vue.nextTick()
	assert.equal(state.current.value.projectId, 'project-1')
	for (let index = 0; index < 5; index++) {
		t.mock.timers.tick(8000)
		await vue.nextTick()
	}
	assert.equal(state.current.value.projectId, 'project-0')
	state.advance(-1)
	assert.equal(state.current.value.projectId, 'project-5')
})

test('hover, keyboard focus, explicit pause and hidden windows each suspend rotation', async (t) => {
	const { state, visibility } = fixture(t)
	for (const control of [state.hovered, state.focused, state.paused, visibility]) {
		control.value = control === visibility ? 'hidden' : true
		await vue.nextTick()
		const before = state.current.value.projectId
		t.mock.timers.tick(30_000)
		await vue.nextTick()
		assert.equal(state.current.value.projectId, before)
		control.value = control === visibility ? 'visible' : false
		await vue.nextTick()
		t.mock.timers.tick(7999)
		await vue.nextTick()
		assert.equal(state.current.value.projectId, before)
		t.mock.timers.tick(1)
		await vue.nextTick()
		assert.notEqual(state.current.value.projectId, before)
	}
})

test('late provider results retain the displayed project and do not jump to a different pack', async (t) => {
	const { state, items } = fixture(t, 3)
	state.advance(1)
	items.value = [
		items.value[0],
		{ ...items.value[0], projectId: 'new-cf' },
		items.value[1],
		items.value[2],
	]
	await vue.nextTick()
	assert.equal(state.current.value.projectId, 'project-1')
	assert.equal(state.currentIndex.value, 2)
})

test('unmount stops pending rotation and a one-project result has no automatic changes', async (t) => {
	const { state, unmount } = fixture(t)
	unmount()
	t.mock.timers.tick(30_000)
	assert.equal(state.current.value.projectId, 'project-0')
})

test('an empty or single-project result never rotates', async (t) => {
	const { state, items } = fixture(t, 1)
	t.mock.timers.tick(30_000)
	assert.equal(state.current.value.projectId, 'project-0')
	items.value = []
	await vue.nextTick()
	t.mock.timers.tick(30_000)
	state.advance(1)
	assert.equal(state.current.value, undefined)
})
