import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { test } from 'node:test'

import * as vue from 'vue'

const ts = createRequire(import.meta.url)('typescript')
const source = readFileSync(
	new URL('../../../packages/ui/src/utils/search.ts', import.meta.url),
	'utf8',
)
const imports = {
	vue,
	'vue-router': { useRoute: () => ({ query: {} }) },
	'@orbiont/assets': new Proxy(
		{},
		{ get: (_target, key) => (key === 'getCategoryIcon' ? (name) => name : {}) },
	),
	'@orbiont/utils': { sortedCategories: (tags) => tags.categories },
	'../composables/i18n': {
		defineMessage: (value) => value,
		useVIntl: () => ({ formatMessage: (value) => value.defaultMessage, locale: vue.ref('en') }),
	},
	'./auto-icons': { getProjectTypeIcon: () => ({}) },
	'./common-messages': {},
	'./disclosures': {
		PROJECT_DISCLOSURE_TYPES: [],
		AI_USAGE_TYPES: [],
		TELEMETRY_CONSENT_TYPES: [],
		isDisclosureCompatibleWithProjectTypes: () => false,
	},
	'./tag-messages.ts': {
		formatCategory: (_format, name) => name,
		formatCategoryHeader: (_format, name) => name,
		DEFAULT_MOD_LOADERS: [],
		DEFAULT_PLUGIN_LOADERS: [],
		DEFAULT_SHADER_LOADERS: [],
	},
}
const module = { exports: {} }
new Function(
	'require',
	'exports',
	ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
	}).outputText,
)((name) => {
	assert.ok(name in imports, `Unexpected import ${name}`)
	return imports[name]
}, module.exports)

test('Worlds exposes Minecraft versions and retains them in filtering and navigation', () => {
	const search = module.exports.useSearch(
		vue.ref(['world']),
		vue.ref({
			gameVersions: [
				{ version: '1.21.1', version_type: 'release' },
				{ version: '24w01a', version_type: 'snapshot' },
			],
			loaders: [],
			categories: [{ name: 'Survival', project_type: 'world', header: 'categories' }],
		}),
		vue.ref([]),
	)
	assert.deepEqual(
		search.filters.value.map((filter) => filter.id),
		['category_world_categories', 'game_version'],
	)
	const versions = search.filters.value.find((filter) => filter.id === 'game_version')
	assert.equal(versions.searchable, true)
	assert.equal(versions.options[1].toggle_group, 'all_versions')
	search.currentFilters.value = [{ type: 'game_version', option: '1.21.1', negative: false }]
	assert.match(search.newFilters.value, /game_versions = `1\.21\.1`/)
	assert.deepEqual(search.createPageParams().v, ['1.21.1'])
})
