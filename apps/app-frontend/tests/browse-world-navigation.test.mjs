import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { test } from 'node:test'

import * as vue from 'vue'

const ts = createRequire(import.meta.url)('typescript')
const browse = readFileSync(new URL('../src/pages/Browse.vue', import.meta.url), 'utf8')
function section(start, end) {
	return browse.slice(browse.indexOf(start), browse.indexOf(end, browse.indexOf(start)))
}
function execute(code, bindings) {
	const names = Object.keys(bindings).filter(
		(name) => name !== 'default' && /^[a-zA-Z_$][\w$]*$/.test(name),
	)
	return new Function(
		...names,
		ts.transpileModule(code, {
			compilerOptions: { target: ts.ScriptTarget.ES2022 },
		}).outputText,
	)(...names.map((name) => bindings[name]))
}

test('the breadcrumb route follows category changes immediately without waiting for search or a render tick', () => {
	const scope = vue.effectScope()
	try {
		const router = {
			currentRoute: vue.shallowRef({ path: '/browse/modpack', params: { projectType: 'modpack' } }),
		}
		const displayed = scope.run(() =>
			execute(
				`${section('const displayedBrowseRoute', 'const breadcrumbMessages')}\nreturn displayedBrowseRoute`,
				{ router, ...vue },
			),
		)
		router.currentRoute.value = {
			path: '/browse/resourcepack',
			params: { projectType: 'resourcepack' },
		}
		assert.equal(displayed.value.params.projectType, 'resourcepack')
		router.currentRoute.value = { path: '/project/test', params: {} }
		assert.equal(displayed.value.params.projectType, 'resourcepack')
	} finally {
		scope.stop()
	}
})

test('Worlds remains visible while browsing Modrinth and its route selects CurseForge', () => {
	const links = execute(
		`${section('const selectableProjectTypes', 'const installContext')}\nreturn selectableProjectTypes.value`,
		{
			...vue,
			instance: vue.ref(null),
			bedrockBrowse: false,
			availableGameVersions: vue.ref([]),
			route: { query: {} },
			contentSource: vue.ref('modrinth'),
			formatMessage: (value) => value,
			messages: { worldsProjectType: 'Worlds' },
			URLSearchParams,
		},
	)
	const worlds = links.find((link) => link.label === 'Worlds')
	assert.ok(worlds)
	assert.notEqual(worlds.shown, false)
	assert.equal(worlds.href, '/browse/world?src=curseforge')
})

test('Worlds switches provider and keeps Modrinth visible but prevents selecting it', async () => {
	const scope = vue.effectScope()
	try {
		const projectType = vue.ref('modpack')
		const contentSource = vue.ref('modrinth')
		const bindings = {
			...vue,
			bedrockBrowse: false,
			projectType,
			contentSource,
			clearSourceSpecificFilters: () => {},
			searchState: { currentPage: vue.ref(1), refreshSearch: async () => {} },
		}
		const tabs = scope.run(() =>
			execute(
				`${section('const contentSourceLinks', 'function clearSourceSpecificFilters')}\n${section('function onContentSourceTabClick', 'watch(supportsCurseforge')}\nreturn { contentSourceLinks, contentSourceIndex, onContentSourceTabClick }`,
				bindings,
			),
		)
		projectType.value = 'world'
		assert.equal(contentSource.value, 'curseforge')
		assert.equal(tabs.contentSourceLinks.value.length, 2)
		assert.equal(tabs.contentSourceLinks.value[0].disabled, true)
		assert.equal(tabs.contentSourceIndex.value, 1)
		tabs.onContentSourceTabClick(0, tabs.contentSourceLinks.value[0])
		assert.equal(contentSource.value, 'curseforge')
		projectType.value = 'datapack'
		assert.equal(tabs.contentSourceLinks.value[0].disabled, false)
		tabs.onContentSourceTabClick(0, tabs.contentSourceLinks.value[0])
		assert.equal(contentSource.value, 'modrinth')
	} finally {
		scope.stop()
	}
})
