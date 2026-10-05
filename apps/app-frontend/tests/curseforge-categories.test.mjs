import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { after, before, test } from 'node:test'

import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { createServer } from 'vite'
import { createApp, effectScope, nextTick, ref, watch } from 'vue'

const classes = { modpack: 4471, mod: 6, resourcepack: 12, datapack: 6945, shader: 6552, world: 17 }
let server
let useCurseforgeCategories
let invoke
let iconAliases
const previousWindow = globalThis.window

before(async () => {
	// Only the native HTTP boundary is simulated; use the real mapping and query composable.
	globalThis.window = { __TAURI_INTERNALS__: { invoke: (...args) => invoke(...args) } }
	server = await createServer({
		configFile: false,
		root: resolve(import.meta.dirname, '..'),
		cacheDir: join(tmpdir(), 'orbiont-category-tests-vite'),
		resolve: { alias: { '@': resolve(import.meta.dirname, '../src') } },
		optimizeDeps: { noDiscovery: true, include: [] },
		ssr: { noExternal: ['@orbiont/assets'] },
		server: { middlewareMode: true, hmr: false },
		appType: 'custom',
		plugins: [
			{
				name: 'category-icon-test-stub',
				enforce: 'pre',
				resolveId: (id) => (id === '@orbiont/assets' ? '\0category-icons' : undefined),
				load: (id) =>
					id === '\0category-icons'
						? 'export let aliases; export const registerCategoryIconAliases = (value) => { aliases = value }'
						: undefined,
			},
		],
	})
	;({ useCurseforgeCategories } = await server.ssrLoadModule(
		'/src/composables/use-curseforge-categories.ts',
	))
	iconAliases = (await server.ssrLoadModule('@orbiont/assets')).aliases
})

test('all Worlds categories resolve to available launcher icons', () => {
	const generated = readFileSync(
		new URL('../../../packages/assets/generated-icons.ts', import.meta.url),
		'utf8',
	)
	const categoryMap = generated
		.split('export const categoryIconMap:')[1]
		.split('export const loaderIconMap:')[0]
	for (const name of [
		'Adventure',
		'Creation',
		'Game Map',
		'Modded World',
		'Parkour',
		'Puzzle',
		'Survival',
	]) {
		const key = iconAliases[name.toLowerCase()]
		assert.ok(key, `Missing icon for ${name}`)
		assert.ok(
			categoryMap.includes(`${key}:`) || categoryMap.includes(`'${key}':`),
			`Unknown icon ${key}`,
		)
	}
})

test('Bedrock category names resolve to available launcher icons', () => {
	const categoryMap = readFileSync(
		new URL('../../../packages/assets/generated-icons.ts', import.meta.url),
		'utf8',
	)
		.split('export const categoryIconMap:')[1]
		.split('export const loaderIconMap:')[0]
	for (const name of [
		'Texture Packs',
		'PvP',
		'Players',
		'Maps',
		'Roleplay',
		'Skins',
		'Cosmetics',
		'Minecraft Addon Maker',
		'Shaders',
		'GUI',
		'l 3D Packs',
		'Enhanced Visuals',
		'Rollercoaster',
		'CTM',
		'Custom Terrain',
		'Scripts',
	]) {
		const key = iconAliases[name.toLowerCase()] ?? name.toLowerCase()
		assert.ok(
			categoryMap.includes(`${key}:`) || categoryMap.includes(`'${key}':`),
			`Missing Bedrock icon for ${name}`,
		)
	}
})

after(async () => {
	await server?.close()
	if (previousWindow === undefined) delete globalThis.window
	else globalThis.window = previousWindow
})

function session(t, type = 'modpack', active = true) {
	const client = new QueryClient()
	const app = createApp({})
	app.use(VueQueryPlugin, { queryClient: client })
	const scope = effectScope()
	const projectType = ref(type)
	const enabled = ref(active)
	const query = app.runWithContext(() =>
		scope.run(() => useCurseforgeCategories(projectType, enabled)),
	)
	t.after(() => {
		scope.stop()
		client.clear()
		client.unmount()
	})
	return { query, projectType, enabled }
}

function response(classId) {
	return { data: [{ id: classId + 1, name: `Category ${classId}`, classId }] }
}

function settled(query) {
	return new Promise((resolve, reject) => {
		const timeout = setTimeout(() => {
			stop()
			reject(new Error('Category query did not settle'))
		}, 7000)
		const stop = watch(
			query.fetchStatus,
			(status) => {
				if (status === 'idle') {
					clearTimeout(timeout)
					stop()
					resolve()
				}
			},
			{ flush: 'post' },
		)
		if (query.fetchStatus.value === 'idle') {
			clearTimeout(timeout)
			stop()
			resolve()
		}
	})
}

test('every supported tab requests its own class and maps category tags', async (t) => {
	const requested = []
	invoke = async (command, { path, query }) => {
		assert.equal(command, 'plugin:orbiont|orbiont_curseforge_api')
		assert.equal(path, 'categories')
		const classId = Number(Object.fromEntries(query).classId)
		requested.push(classId)
		return response(classId)
	}
	const { projectType, query } = session(t)
	for (const [type, classId] of Object.entries(classes)) {
		projectType.value = type
		await nextTick()
		await settled(query)
		assert.equal(query.data.value.length, 1)
		assert.equal(query.data.value[0].project_type, type)
		assert.equal(query.data.value[0].name, `Category ${classId}`)
	}
	assert.deepEqual(requested, Object.values(classes))
})

test('late responses cannot replace categories for the active tab', async (t) => {
	const pending = new Map()
	invoke = (_command, { query }) =>
		new Promise((resolve) => pending.set(Number(Object.fromEntries(query).classId), resolve))
	const { projectType, query } = session(t)
	projectType.value = 'shader'
	await nextTick()
	assert.equal(query.data.value, undefined)
	pending.get(classes.shader)(response(classes.shader))
	await settled(query)
	pending.get(classes.modpack)(response(classes.modpack))
	await new Promise((resolve) => setTimeout(resolve, 10))
	assert.equal(query.data.value[0].project_type, 'shader')
})

test('provider changes during navigation load the current tab', async (t) => {
	const requested = []
	invoke = async (_command, { query }) => {
		const classId = Number(Object.fromEntries(query).classId)
		requested.push(classId)
		return response(classId)
	}
	const { projectType, enabled, query } = session(t, 'modpack', false)
	assert.deepEqual(requested, [])
	enabled.value = true
	projectType.value = 'datapack'
	await nextTick()
	await settled(query)
	assert.deepEqual(requested, [classes.datapack])
	assert.equal(query.data.value[0].project_type, 'datapack')
})

test('returning to a pending tab still receives its categories', async (t) => {
	const pending = new Map()
	invoke = (_command, { query }) =>
		new Promise((resolve) => pending.set(Number(Object.fromEntries(query).classId), resolve))
	const { projectType, query } = session(t)
	projectType.value = 'mod'
	await nextTick()
	projectType.value = 'modpack'
	await nextTick()
	pending.get(classes.mod)(response(classes.mod))
	await new Promise((resolve) => setTimeout(resolve, 10))
	assert.equal(query.data.value, undefined)
	pending.get(classes.modpack)(response(classes.modpack))
	await settled(query)
	assert.equal(query.data.value[0].project_type, 'modpack')
})

test('a temporary failure retries instead of permanently hiding the filter', async (t) => {
	let attempts = 0
	invoke = async () => {
		if (++attempts === 1) throw new Error('temporary network failure')
		return response(classes.modpack)
	}
	const { query } = session(t)
	await settled(query)
	assert.equal(attempts, 2)
	assert.equal(query.isSuccess.value, true)
	assert.equal(query.data.value[0].project_type, 'modpack')
})

test('429 does not trigger additional frontend retries after the native cooldown', async (t) => {
	let attempts = 0
	invoke = async () => {
		attempts++
		throw new Error('CurseForge request failed: 429 Too Many Requests')
	}
	const { query } = session(t)
	await settled(query)
	assert.equal(attempts, 1)
	assert.equal(query.isError.value, true)
})

test('after bounded retries, manual retry recovers without restarting the launcher', async (t) => {
	let attempts = 0
	invoke = async () => {
		attempts++
		throw new Error('offline')
	}
	const { query } = session(t)
	await settled(query)
	assert.equal(attempts, 3)
	assert.equal(query.isError.value, true)
	invoke = async () => response(classes.modpack)
	await query.refetch()
	assert.equal(query.isSuccess.value, true)
	assert.equal(query.data.value.length, 1)
})
