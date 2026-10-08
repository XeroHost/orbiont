import assert from 'node:assert/strict'
import test from 'node:test'

import { retentionSelection } from '../src/helpers/recovery-retention.ts'

const now = 100 * 86400
const copy = (id, overrides = {}) => ({
	id,
	rootId: 'root',
	path: 'world',
	created: Number(id) * 86400,
	size: 1,
	limited: false,
	state: 'available',
	removable: true,
	...overrides,
})
const storage = (copies) => copies.map((item) => ({ ...item, category: 'recovery' }))

test('retention keeps newest per destination and requires both age and safe removable state', () => {
	const copies = [
		copy('1'),
		copy('2'),
		copy('3'),
		copy('4', { path: 'another' }),
		copy('5', { state: 'partial' }),
		copy('6', { created: 0 }),
		copy('7', { created: now }),
		copy('8', { limited: true }),
		copy('9', { path: 'installed' }),
	]
	const entries = storage(copies)
	entries.at(-1).category = 'content'
	assert.deepEqual(
		[...retentionSelection(entries, copies, { days: 30, keep: 2 }, now)],
		['root/3', 'root/2', 'root/1'],
	)
})

test('retention rejects invalid policies and never selects unknown or protected copies', () => {
	const copies = [
		copy('1', { state: 'rollback_failed' }),
		copy('2', { removable: false }),
		copy('3'),
	]
	assert.equal(retentionSelection(storage(copies), copies, { days: 0, keep: 1 }, now).size, 0)
	assert.equal(retentionSelection(storage(copies), copies, { days: 1, keep: 1 }, now).size, 0)
})
