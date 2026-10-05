import assert from 'node:assert/strict'
import { test } from 'node:test'

import { matchesOptifineSelection } from '../src/helpers/optifine-selection.ts'

const reference = {
	minecraftVersion: '1.21.1',
	version: 'HD_U_J1',
	installerSha256: 'a'.repeat(64),
}

test('requires the actual game version and Vanilla', () => {
	assert.equal(matchesOptifineSelection(reference, '1.21.1', 'vanilla'), true)
	assert.equal(matchesOptifineSelection(reference, '26.3', 'vanilla'), false)
	for (const loader of ['fabric', 'forge', 'quilt', 'neoforge']) {
		assert.equal(matchesOptifineSelection(reference, '1.21.1', loader), false)
	}
})

test('imported Orbpack must use the exact installer, not merely a similar name', () => {
	assert.equal(matchesOptifineSelection(reference, '1.21.1', 'vanilla', reference), true)
	assert.equal(
		matchesOptifineSelection(
			{ ...reference, installerSha256: 'b'.repeat(64) },
			'1.21.1',
			'vanilla',
			reference,
		),
		false,
	)
	assert.equal(
		matchesOptifineSelection({ ...reference, version: 'HD_U_J2' }, '1.21.1', 'vanilla', reference),
		false,
	)
})
