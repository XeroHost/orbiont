import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { test } from 'node:test'

import { onlineManager, QueryClient } from '@tanstack/vue-query'
import * as vue from 'vue'

test('mixed-provider updates retain native candidates and expose unsupported CurseForge rows before preflight', async () => {
	const { supportsNativeSelectedUpdate } = evaluate(
		'../src/helpers/selected-content-updates.ts',
		{},
	)
	const { partitionBulkUpdateCandidates } = evaluate(
		'../../../packages/ui/src/layouts/shared/content-tab/utils/bulk-update-candidates.ts',
		{},
	)
	const native = {
		project: { id: 'native-project', title: 'Native' },
		version: { id: 'native-old' },
		update_version_id: 'native-new',
	}
	const curseforge = {
		project: { id: 'cf-123', title: 'CurseForge project' },
		version: { id: 'cf-123-1' },
		update_version_id: 'cf-123-2',
	}
	const { supported, unsupported } = partitionBulkUpdateCandidates(
		[curseforge, native],
		supportsNativeSelectedUpdate,
	)
	assert.deepEqual(supported, [native])
	assert.deepEqual(
		unsupported.map((item) => item.project.title),
		['CurseForge project'],
	)
	const { useUpdateAllValidation } = evaluate(
		'../../../packages/ui/src/components/modal/update-all-modal/use-update-all-validation.ts',
		{ vue },
	)
	const selections = supported.map((item) => ({
		id: item.project.id,
		projectId: item.project.id,
		version: { id: item.update_version_id },
	}))
	const preflight = useUpdateAllValidation(() => async (submitted) => {
		assert.deepEqual(submitted, selections)
		assert.equal(
			submitted.some((item) => item.projectId.startsWith('cf-')),
			false,
		)
	})
	assert.equal(await preflight.run(selections), true)
	assert.equal(supportsNativeSelectedUpdate({ ...native, update_version_id: 'cf-123-2' }), false)
})
const ts = createRequire(import.meta.url)('typescript')
function evaluate(path, imports) {
	const exports = {}
	const code = ts.transpileModule(readFileSync(new URL(path, import.meta.url), 'utf8'), {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
	}).outputText
	new Function('require', 'exports', code)((name) => imports[name], exports)
	return exports
}

test('local content and disk synchronization run offline with independent instance cache keys', async () => {
	const calls = []
	const options = evaluate('../src/pages/instance/query-options.ts', {
		'@/helpers/cache.js': {},
		'@/helpers/instance': { sync_content_files: async (id) => calls.push(['sync', id]) },
		'@/helpers/instance-content': {
			loadInstanceContentData: async (id) => {
				calls.push(['read', id])
				return { instanceId: id }
			},
		},
		'@/helpers/process': {},
		'@/helpers/worlds': {},
	})
	const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
	onlineManager.setOnline(false)
	try {
		const first = options.instanceContentQueryOptions('first')
		const second = options.instanceContentQueryOptions('second')
		assert.notDeepEqual(first.queryKey, second.queryKey)
		assert.deepEqual(await client.fetchQuery(first), { instanceId: 'first' })
		assert.deepEqual(await client.fetchQuery(second), { instanceId: 'second' })
		const sync = options.instanceContentSyncQueryOptions('first', async (id) =>
			calls.push(['invalidate', id]),
		)
		assert.notDeepEqual(sync.queryKey, first.queryKey)
		assert.equal(sync.refetchOnWindowFocus, false)
		assert.equal(sync.refetchOnReconnect, false)
		assert.equal(await client.fetchQuery(sync), 'first')
		assert.deepEqual(calls, [
			['read', 'first'],
			['read', 'second'],
			['sync', 'first'],
			['invalidate', 'first'],
		])
	} finally {
		client.clear()
		onlineManager.setOnline(true)
	}
})
test('update selection reconciles versions, subset, reset and missing candidates', async () => {
	const scope = vue.effectScope()
	const items = vue.ref([
		{
			id: 'one',
			project: { id: 'cf-1' },
			currentVersion: { id: 'old' },
			versions: [{ id: 'old' }, { id: 'new' }, { id: 'other' }],
			initialVersionId: 'new',
		},
		{ id: 'two', project: { id: 'mr-2' }, currentVersion: { id: 'installed' }, versions: [] },
	])
	const { useUpdateAllSelection } = evaluate(
		'../../../packages/ui/src/components/modal/update-all-modal/use-update-all-selection.ts',
		{ vue },
	)
	const selection = scope.run(() => useUpdateAllSelection(() => items.value))
	assert.deepEqual(
		selection.selections.value.map((s) => s.version.id),
		['new'],
	)
	selection.selectVersion('one', 'invented')
	assert.equal(selection.selections.value[0].version.id, 'new')
	selection.selectVersion('one', 'other')
	selection.selectAll(false)
	assert.equal(selection.selections.value.length, 0)
	selection.reset()
	assert.equal(selection.selections.value[0].version.id, 'new')
	items.value[0].versions = [{ id: 'other' }]
	await vue.nextTick()
	assert.equal(selection.selections.value[0].version.id, 'other')
	items.value = []
	await vue.nextTick()
	assert.equal(selection.selections.value.length, 0)
	scope.stop()
})
test('custom Java accepts detected JRE, game compatibility remains strict and timers dispose', async () => {
	const calls = []
	const { default: useJavaTest } = evaluate('../src/composables/useJavaTest.ts', {
		vue,
		'@/helpers/jre.js': {
			get_jre: async (path) => {
				calls.push(['detect', path])
				return { parsed_version: 8 }
			},
			test_jre: async (path, version) => {
				calls.push(['compatible', path, version])
				return false
			},
		},
	})
	const scope = vue.effectScope()
	const java = scope.run(useJavaTest)
	await java.testJavaInstallation('custom', null)
	assert.equal(java.javaTestResult.value, true)
	await java.testJavaInstallation('custom', 21)
	assert.equal(java.javaTestResult.value, true)
	assert.equal(java.javaCompatibilityResult.value, false)
	java.testJavaInstallationDebounced('disposed', null, 1)
	scope.stop()
	await new Promise((resolve) => setTimeout(resolve, 10))
	assert.deepEqual(calls, [
		['detect', 'custom'],
		['detect', 'custom'],
		['compatible', 'custom', 21],
	])
})

test('preflight displays dependency errors and canceled validation cannot submit', async () => {
	const { useUpdateAllValidation } = evaluate(
		'../../../packages/ui/src/components/modal/update-all-modal/use-update-all-validation.ts',
		{ vue },
	)
	const selection = [{ id: 'mods/a.jar', projectId: 'cf-1', version: { id: 'cf-1-2' } }]
	const validation = useUpdateAllValidation(() => async (entries) => {
		assert.deepEqual(entries, selection)
		throw new Error('A2 requires B2; installed B1 is frozen')
	})
	assert.equal(await validation.run(selection), false)
	assert.equal(validation.validationError.value, 'A2 requires B2; installed B1 is frozen')
	let finish
	const pending = useUpdateAllValidation(
		() => () =>
			new Promise((resolve) => {
				finish = resolve
			}),
	)
	const result = pending.run(selection)
	assert.equal(pending.validating.value, true)
	pending.reset()
	finish()
	assert.equal(await result, false)
	assert.equal(pending.validationError.value, null)
})
