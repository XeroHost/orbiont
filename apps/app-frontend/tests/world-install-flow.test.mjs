import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { test } from 'node:test'

import dayjs from 'dayjs'
import * as vue from 'vue'

const ts = createRequire(import.meta.url)('typescript')
function evaluate(source, imports) {
	const exports = {}
	const code = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
	}).outputText
	new Function('require', 'exports', code)((name) => {
		if (name in imports) return imports[name]
		throw new Error(`Unexpected import: ${name}`)
	}, exports)
	return exports
}
function fixture({
	type = 'world',
	gameVersion = '1.21.11',
	result = 'cf-version',
	failedCreation = false,
} = {}) {
	const calls = []
	const project = {
		id: 'cf-world',
		project_type: type,
		title: 'Adventure',
		versions: ['cf-version'],
	}
	const version = {
		id: 'cf-version',
		project_id: project.id,
		game_versions: ['1.21.11'],
		loaders: ['datapack'],
		date_published: '2026-09-01',
		files: [],
	}
	const instance = {
		id: 'instance',
		name: 'Test',
		loader: 'fabric',
		game_version: gameVersion,
		install_stage: 'installed',
	}
	let finishCreation
	const creation = new Promise((resolve, reject) => {
		finishCreation = failedCreation
			? () => reject(new Error('Creation failed'))
			: () => resolve({ instance_id: 'new-instance' })
	})
	const module = evaluate(
		readFileSync(new URL('../src/providers/content-install.ts', import.meta.url), 'utf8'),
		{
			vue,
			dayjs: { default: dayjs },
			'@orbiont/ui': {
				createContext: () => [() => {}, () => {}],
				defineMessage: (value) => value,
				useVIntl: () => ({ formatMessage: (value) => value.defaultMessage }),
			},
			'@tauri-apps/plugin-opener': {},
			'@/composables/use-app-settings.ts': {
				useAppSettings: () => ({ getFeatureFlag: () => false }),
			},
			'@/config': { config: {} },
			'@/helpers/bedrock-catalog': {
				requestBedrockInstall: async () => {
					throw new Error('Java worlds must never use the Bedrock installer')
				},
			},
			'@/helpers/cache.js': {
				get_project: async () => project,
				get_version_many: async () => [version],
				get_project_many: async () => [],
			},
			'@/helpers/curseforge': {
				isCurseforgeId: () => true,
				getCurseforgeProjectVersions: async () => [version],
			},
			'@/helpers/install': {
				install_create_instance: async (data) => {
					calls.push(['create', data])
					return { job_id: 'job' }
				},
				wait_for_install_job: async () => {
					calls.push(['wait'])
					return creation
				},
				installJobInstanceId: (job) => job.instance_id,
			},
			'@/helpers/instance': {
				list: async () => [instance],
				get: async () => instance,
				getInstanceIconUrl: () => null,
				get_install_candidates: async () => [{ ...instance, installed: false, compatible: true }],
				install_project_with_dependencies: async (id, request) => {
					calls.push(['dependencies', id, request])
					return { primary: { project_id: project.id, version_id: version.id }, dependencies: [] }
				},
				add_project_from_version: async () => {
					throw new Error('Worlds must never use the mods installer')
				},
			},
			'@/helpers/tags': {
				get_game_versions: async () => [{ version: '1.21.11', version_type: 'release' }],
			},
			'@/helpers/world-install': {
				requestWorldInstall: async (request) => {
					calls.push(['world', request])
					return result
				},
			},
		},
	)
	const scope = vue.effectScope()
	const context = scope.run(() =>
		module.createContentInstall({
			router: { push: async () => {} },
			appEvents: { on: () => () => {} },
			handleError: (error) => calls.push(['error', error.message]),
		}),
	)
	context.setContentInstallModal({
		show: () => calls.push(['show']),
		hide: () => context.handleCancel(),
	})
	context.setIncompatibilityWarningModal({
		show: () => calls.push(['warning']),
		hide: () => context.handleIncompatibilityWarningCancel(),
	})
	return { context, calls, scope, project, version, finishCreation }
}

test('worlds use the regular existing/new instance selector before importing', async () => {
	const f = fixture()
	try {
		await f.context.install(f.project.id)
		assert.equal(f.calls[0][0], 'show')
		assert.equal(
			f.calls.some(([name]) => name === 'world'),
			false,
		)
		assert.equal(f.context.instances.value[0].compatible, true)
		assert.equal(f.context.instances.value[0].installed, false)
		assert.equal(f.context.compatibleLoaders.value[0], 'vanilla')
		await f.context.handleInstallToInstance(f.context.instances.value[0])
		assert.equal(f.calls.at(-1)[0], 'world')
		assert.equal(f.calls.at(-1)[1].instanceId, 'instance')
		assert.equal(f.calls.at(-1)[1].autoInstall, true)
		assert.equal(
			f.calls.some(([name]) => name === 'dependencies'),
			false,
		)
	} finally {
		f.scope.stop()
	}
})

test('creating an instance waits for successful creation before installing the world', async () => {
	const f = fixture()
	const completion = []
	try {
		await f.context.install(f.project.id, null, null, 'test', (value) => completion.push(value))
		const installing = f.context.handleCreateAndInstall({
			name: 'Adventure',
			gameVersion: '1.21.11',
			loader: 'vanilla',
			iconPath: null,
		})
		f.context.handleCancel() // The shared modal hides after emitting create-and-install.
		await vue.nextTick()
		assert.deepEqual(completion, [])
		assert.equal(
			f.calls.some(([name]) => name === 'world'),
			false,
		)
		f.finishCreation()
		await installing
		assert.equal(f.calls.at(-1)[1].instanceId, 'new-instance')
		assert.deepEqual(completion, ['cf-version'])
	} finally {
		f.scope.stop()
	}
})

test('failed instance creation never imports a world', async () => {
	const f = fixture({ failedCreation: true })
	try {
		await f.context.install(f.project.id)
		const installing = f.context.handleCreateAndInstall({
			name: 'Adventure',
			gameVersion: '1.21.11',
			loader: 'vanilla',
			iconPath: null,
		})
		f.finishCreation()
		await installing
		assert.equal(
			f.calls.some(([name]) => name === 'world'),
			false,
		)
		assert.equal(f.calls.at(-1)[0], 'error')
	} finally {
		f.scope.stop()
	}
})

test('an incompatible existing instance uses the shared version warning and cancellation reports no success', async () => {
	const f = fixture({ gameVersion: '26.3', result: null })
	const completion = []
	try {
		await f.context.install(f.project.id, null, null, 'test', (value) => completion.push(value))
		await f.context.handleInstallToInstance(f.context.instances.value[0])
		assert.equal(f.calls.at(-1)[0], 'warning')
		assert.deepEqual(completion, [])
		await f.context.handleIncompatibilityWarningInstall(f.version)
		assert.equal(f.calls.at(-1)[0], 'world')
		assert.equal(f.calls.at(-1)[1].allowIncompatible, true)
		assert.deepEqual(completion, [undefined])
	} finally {
		f.scope.stop()
	}
})

test('Data Packs continue to use the existing dependency installer', async () => {
	const f = fixture({ type: 'datapack' })
	try {
		await f.context.install(f.project.id)
		await f.context.handleInstallToInstance(f.context.instances.value[0])
		assert.equal(
			f.calls.some(([name]) => name === 'world'),
			false,
		)
		assert.equal(f.calls.find(([name]) => name === 'dependencies')[2].content_type, 'datapack')
	} finally {
		f.scope.stop()
	}
})
