import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
	getPackImportFilters,
	getPackSaveFilters,
	resolvePackExport,
} from '../src/helpers/pack-formats.ts'

test('import exposes all formats with short individual provider filters', () => {
	const filters = getPackImportFilters('Modpacks')
	assert.deepEqual(
		filters.map((filter) => filter.extensions),
		[['orbpack', 'mrpack', 'zip'], ['orbpack'], ['mrpack'], ['zip']],
	)
	assert.deepEqual(
		filters.map((filter) => filter.name),
		['Modpacks', 'Orbiont', 'Modrinth', 'CurseForge'],
	)
})

test('save offers all three formats and puts the selected one first', () => {
	assert.deepEqual(
		getPackSaveFilters('orbpack').map((filter) => filter.extensions[0]),
		['orbpack', 'mrpack', 'zip'],
	)
	assert.deepEqual(
		getPackSaveFilters('curseforge').map((filter) => filter.extensions[0]),
		['zip', 'orbpack', 'mrpack'],
	)
})

test('the returned extension determines the actual archive format', () => {
	assert.deepEqual(resolvePackExport('C:/packs/Pack.zip', 'orbpack'), {
		path: 'C:/packs/Pack.zip',
		format: 'curseforge',
	})
	assert.deepEqual(resolvePackExport('Pack.MRPACK', 'curseforge'), {
		path: 'Pack.MRPACK',
		format: 'mrpack',
	})
	assert.deepEqual(resolvePackExport('Pack', 'orbpack'), {
		path: 'Pack.orbpack',
		format: 'orbpack',
	})
})
