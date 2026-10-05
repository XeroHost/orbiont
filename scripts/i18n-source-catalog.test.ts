import assert from 'node:assert/strict'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'

import { coveragePercentage, sourceMessages } from './i18n-source-catalog.ts'

test('coverage sees new Vue and helper messages before catalog extraction', () => {
	const directory = mkdtempSync(join(tmpdir(), 'orbiont-source-messages-'))
	try {
		writeFileSync(
			join(directory, 'world.vue'),
			`<script setup lang="ts">const messages = defineMessages({ title: { id: 'world.title', defaultMessage: 'Install world' } })</script>`,
		)
		writeFileSync(
			join(directory, 'tags.ts'),
			`export const message = { id: 'world.category', defaultMessage: 'Worlds' }`,
		)
		assert.deepEqual(sourceMessages([directory]), {
			'world.title': { defaultMessage: 'Install world' },
			'world.category': { defaultMessage: 'Worlds' },
		})
	} finally {
		rmSync(directory, { recursive: true, force: true })
	}
})

test('even one untranslated message prevents a 100% label', () => {
	assert.equal(coveragePercentage(2999, 3000), 99)
	assert.equal(coveragePercentage(3000, 3000), 100)
	assert.equal(coveragePercentage(0, 0), 100)
})
