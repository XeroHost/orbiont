import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { test } from 'node:test'

const ts = createRequire(import.meta.url)('typescript')
const file = new URL('../src/helpers/featured-modpacks.ts', import.meta.url)
const implementation = existsSync(file) ? readFileSync(file, 'utf8') : ''
const exports = {}
new Function(
	'exports',
	ts.transpileModule(implementation, {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
	}).outputText,
)(exports)
const { selectFeaturedModpacks } = exports
const hit = (project_id, name, slug = project_id, extra = {}) => ({
	project_id,
	name,
	slug,
	summary: `${name} summary`,
	icon_url: null,
	featured_gallery: null,
	gallery: [],
	...extra,
})

test('selects three distinct ranked modpacks per provider and alternates providers', () => {
	assert.equal(typeof selectFeaturedModpacks, 'function', 'Featured selection is not implemented')
	const result = selectFeaturedModpacks(
		[hit('m1', 'First'), hit('m2', 'Second'), hit('m3', 'Third'), hit('m4', 'Fourth')],
		[hit('cf-1', 'Fifth'), hit('cf-2', 'Sixth'), hit('cf-3', 'Seventh'), hit('cf-4', 'Eighth')],
	)
	assert.deepEqual(
		result.map((item) => [item.projectId, item.provider]),
		[
			['m1', 'modrinth'],
			['cf-1', 'curseforge'],
			['m2', 'modrinth'],
			['cf-2', 'curseforge'],
			['m3', 'modrinth'],
			['cf-3', 'curseforge'],
		],
	)
})

test('skips equivalent names across providers and fills CurseForge places from later results', () => {
	assert.equal(typeof selectFeaturedModpacks, 'function')
	const result = selectFeaturedModpacks(
		[hit('m1', 'Fabulously Optimized'), hit('m2', 'All the Mods 10'), hit('m3', 'Unique pack')],
		[
			hit('cf-1', 'FABULOUSLY—OPTIMIZED'),
			hit('cf-2', 'All the Mods 10 - ATM10'),
			hit('cf-3', 'Different'),
			hit('cf-4', 'Another'),
			hit('cf-5', 'Last'),
		],
	)
	assert.deepEqual(
		result.map((item) => item.projectId),
		['m1', 'cf-3', 'm2', 'cf-4', 'm3', 'cf-5'],
	)
})

test('uses provider slugs to identify a duplicated project with a different display name', () => {
	assert.equal(typeof selectFeaturedModpacks, 'function')
	const result = selectFeaturedModpacks(
		[hit('m1', 'Pack with a subtitle', 'shared-pack')],
		[
			hit('cf-1', 'Pack', 'cf-1', {
				page_url: 'https://www.curseforge.com/minecraft/modpacks/shared-pack',
			}),
			hit('cf-2', 'Separate pack'),
		],
	)
	assert.deepEqual(
		result.map((item) => item.projectId),
		['m1', 'cf-2'],
	)
})

test('preserves different numbered editions and does not collapse missing slugs', () => {
	assert.equal(typeof selectFeaturedModpacks, 'function')
	const result = selectFeaturedModpacks(
		[hit('m1', 'Prominence II', '')],
		[hit('cf-1', 'Prominence III', ''), hit('cf-2', 'Other pack', '')],
	)
	assert.deepEqual(
		result.map((item) => item.projectId),
		['m1', 'cf-1', 'cf-2'],
	)
})

test('handles an unavailable provider without inventing extra slots or displaying duplicate hits', () => {
	assert.equal(typeof selectFeaturedModpacks, 'function')
	const result = selectFeaturedModpacks(
		[],
		[hit('cf-1', 'Pack'), hit('cf-1', 'Pack'), hit('cf-2', 'Other'), hit('cf-3', 'Last')],
	)
	assert.deepEqual(
		result.map((item) => item.projectId),
		['cf-1', 'cf-2', 'cf-3'],
	)
	assert.deepEqual(selectFeaturedModpacks([], []), [])
})

test('keeps a featured gallery image for the card and falls back to the icon', () => {
	assert.equal(typeof selectFeaturedModpacks, 'function')
	const result = selectFeaturedModpacks(
		[
			hit('m1', 'Pack', 'pack', {
				featured_gallery: 'https://cdn.modrinth.com/cover.png',
				icon_url: 'https://cdn.modrinth.com/icon.png',
			}),
			hit('m2', 'Other', 'other', { icon_url: 'https://cdn.modrinth.com/icon2.png' }),
		],
		[],
	)
	assert.equal(result[0].imageUrl, 'https://cdn.modrinth.com/cover.png')
	assert.equal(result[1].imageUrl, 'https://cdn.modrinth.com/icon2.png')
})
