import assert from 'node:assert/strict'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { after, before, test } from 'node:test'

import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { createServer } from 'vite'
import { createApp, effectScope, watch } from 'vue'

let server
let useFeaturedModpacks
let invoke
const previousWindow = globalThis.window

before(async () => {
	globalThis.window = { __TAURI_INTERNALS__: { invoke: (...args) => invoke(...args) } }
	server = await createServer({
		configFile: false,
		root: resolve(import.meta.dirname, '..'),
		cacheDir: join(tmpdir(), 'orbiont-featured-query-tests-vite'),
		resolve: { alias: { '@': resolve(import.meta.dirname, '../src') } },
		optimizeDeps: { noDiscovery: true, include: [] },
		server: { middlewareMode: true, hmr: false },
		appType: 'custom',
		plugins: [
			{
				name: 'category-icon-boundary',
				resolveId: (id) => (id === '@orbiont/assets' ? '\0category-icons' : undefined),
				load: (id) =>
					id === '\0category-icons'
						? 'export const registerCategoryIconAliases = () => {}'
						: undefined,
			},
		],
	})
	;({ useFeaturedModpacks } = await server.ssrLoadModule(
		'/src/composables/use-featured-modpacks.ts',
	))
})

after(async () => {
	await server?.close()
	if (previousWindow === undefined) delete globalThis.window
	else globalThis.window = previousWindow
})

function session(t) {
	const client = new QueryClient()
	const app = createApp({})
	app.use(VueQueryPlugin, { queryClient: client })
	const scope = effectScope()
	const query = app.runWithContext(() => scope.run(useFeaturedModpacks))
	t.after(() => {
		scope.stop()
		client.clear()
		client.unmount()
	})
	return query
}

function settled(query) {
	return new Promise((resolve, reject) => {
		let stop = () => {}
		const timeout = setTimeout(() => {
			stop()
			reject(new Error('Featured queries did not settle'))
		}, 7000)
		const finish = () => {
			clearTimeout(timeout)
			stop()
			resolve()
		}
		stop = watch(
			query.loading,
			(loading) => {
				if (!loading) finish()
			},
			{ flush: 'post' },
		)
		if (!query.loading.value) finish()
	})
}

const modrinthHits = ['Shared pack', 'Second pack', 'Third pack'].map((name, index) => ({
	project_id: `m${index}`,
	name,
	slug: `pack-${index}`,
	summary: 'Summary',
	gallery: [],
	icon_url: null,
	featured_gallery: null,
}))
const curseforgeMods = ['Shared pack', 'CF first', 'CF second', 'CF third'].map((name, index) => ({
	id: index + 1,
	name,
	slug: `cf-pack-${index}`,
	summary: 'Summary',
	classId: 4471,
	links: { websiteUrl: `https://www.curseforge.com/minecraft/modpacks/cf-pack-${index}` },
	categories: [],
	authors: [],
	latestFilesIndexes: [],
	latestFiles: [],
	logo: null,
	screenshots: [],
	downloadCount: 100 - index,
	dateCreated: '2026-01-01T00:00:00Z',
	dateModified: '2026-01-01T00:00:00Z',
	dateReleased: '2026-01-01T00:00:00Z',
	allowModDistribution: true,
}))

test('native provider searches feed six unique cards and duplicate replacements without per-slide requests', async (t) => {
	const calls = []
	invoke = async (command, args) => {
		calls.push(command)
		if (command === 'plugin:cache|get_search_results_v3') {
			const params = new URLSearchParams(args.id)
			assert.deepEqual(JSON.parse(params.get('facets')), [['project_type:modpack']])
			assert.equal(params.get('index'), 'relevance')
			assert.equal(params.get('limit'), '12')
			return { result: { hits: modrinthHits } }
		}
		assert.equal(command, 'plugin:orbiont|orbiont_curseforge_api')
		assert.equal(args.path, 'mods/search')
		const params = Object.fromEntries(args.query)
		assert.equal(params.classId, '4471')
		assert.equal(params.sortField, '1')
		assert.equal(params.pageSize, '12')
		return { data: curseforgeMods, pagination: { totalCount: 4 } }
	}
	const query = session(t)
	await settled(query)
	assert.deepEqual(
		query.items.value.map((item) => item.projectId),
		['m0', 'cf-2', 'm1', 'cf-3', 'm2', 'cf-4'],
	)
	for (let index = 0; index < 20; index++) assert.equal(query.items.value.length, 6)
	assert.equal(calls.length, 2)
})

test('a CurseForge cooldown leaves Modrinth cards available and does not retry 429', async (t) => {
	let attempts = 0
	invoke = async (command) => {
		if (command === 'plugin:cache|get_search_results_v3') return { result: { hits: modrinthHits } }
		attempts++
		throw new Error('CurseForge request failed: 429 Too Many Requests')
	}
	const query = session(t)
	await settled(query)
	assert.equal(attempts, 1)
	assert.deepEqual(
		query.items.value.map((item) => item.projectId),
		['m0', 'm1', 'm2'],
	)
})
