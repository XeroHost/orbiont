import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { test } from 'node:test'

import { compileScript, parse } from '@vue/compiler-sfc'
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
class CurseforgeDownloadCancelled extends Error {}
function fixture({ gameVersion = '1.21.1', collision = true, downloadError } = {}) {
	const calls = []
	const conflict = { path: 'world', name: 'Adventure', fingerprint: 'original' }
	const preview = { folder: 'world', name: 'Adventure', conflicts: collision ? [conflict] : [] }
	const native = evaluate(
		readFileSync(new URL('../src/helpers/world-install.ts', import.meta.url), 'utf8'),
		{
			'@tauri-apps/api/core': {
				invoke: async (command, payload) => {
					calls.push([command, payload])
					return command.endsWith('inspect_world_archive')
						? preview
						: {
								path: payload.action === 'copy' ? 'world 2' : 'world',
								backup: payload.action === 'replace' ? 'backups/world' : null,
							}
				},
			},
		},
	)
	const { descriptor } = parse(
		readFileSync(
			new URL('../src/components/ui/world/modal/InstallWorldModal.vue', import.meta.url),
			'utf8',
		),
	)
	const component = evaluate(compileScript(descriptor, { id: 'world-install-test' }).content, {
		vue: { ...vue, onUnmounted: () => {} },
		'@orbiont/assets': {},
		'@orbiont/ui': {
			defineMessages: (value) => value,
			useVIntl: () => ({ formatMessage: (value) => value.defaultMessage }),
			injectNotificationManager: () => ({
				handleError: (error) => {
					throw error
				},
			}),
		},
		'@tanstack/vue-query': { useQueryClient: () => ({ invalidateQueries: async () => {} }) },
		'@/helpers/curseforge': {
			CurseforgeDownloadCancelled,
			downloadVersionFile: async () => {
				calls.push(['download'])
				if (downloadError) throw downloadError
				return '/verified/world.zip'
			},
		},
		'@/helpers/instance': {
			list: async () => [
				{ id: 'instance-1', name: 'Test', game_version: gameVersion, install_stage: 'installed' },
			],
		},
		'@/helpers/world-install': native,
		'@/pages/instance/query-options': { instanceKeys: { worlds: (id) => ['worlds', id] } },
		'@/components/ui/world/queries': { recentWorldsKey: ['worlds', 'recent'] },
	}).default
	const scope = vue.effectScope()
	const installer = scope.run(() => component.setup({}, { expose: () => {}, emit: () => {} }))
	let modalShows = 0
	installer.modal.value = { show: () => modalShows++, hide: () => installer.settle() }
	const request = {
		project: { id: 'cf-123', title: 'Adventure', project_type: 'world' },
		versions: [{ id: 'cf-123-456', game_versions: ['1.21.1'], name: 'Release' }],
		instanceId: 'instance-1',
	}
	return { installer, calls, scope, request, conflict, modalShows: () => modalShows }
}
async function ready(installer) {
	for (let index = 0; index < 10 && installer.busy.value; index++) await vue.nextTick()
	assert.equal(installer.busy.value, false)
}

test('a collision prompts before modifying saves; another copy passes the explicit action', async () => {
	const { installer, calls, scope, request } = fixture()
	try {
		const completion = installer.show(request)
		await ready(installer)
		await installer.install()
		assert.equal(installer.preview.value.conflicts[0].path, 'world')
		assert.equal(calls.filter(([command]) => command.endsWith('import_world_archive')).length, 0)
		await installer.install('copy')
		assert.equal(await completion, 'cf-123-456')
		assert.equal(installer.installedPath.value, 'world 2')
		assert.equal(calls.filter(([command]) => command === 'download').length, 1)
		assert.equal(calls.at(-1)[1].action, 'copy')
	} finally {
		scope.stop()
	}
})

test('canceling the manual download ends the world flow without an error or recovery modal', async () => {
	const f = fixture({ downloadError: new CurseforgeDownloadCancelled() })
	try {
		const result = await f.installer.show({ ...f.request, autoInstall: true })
		assert.equal(result, null)
		assert.deepEqual(f.calls, [['download']])
		assert.equal(f.modalShows(), 0)
		assert.equal(f.installer.installedPath.value, '')
	} finally {
		f.scope.stop()
	}
})

test('Keep existing and Cancel resolve without installing or reporting success', async () => {
	for (const choice of ['keep', 'cancel']) {
		const { installer, calls, scope, request } = fixture()
		try {
			const completion = installer.show(request)
			await ready(installer)
			await installer.install()
			// Both displayed actions close through the same cancellation handler.
			installer.cancel(choice)
			assert.equal(await completion, null)
			assert.equal(calls.filter(([command]) => command.endsWith('import_world_archive')).length, 0)
		} finally {
			scope.stop()
		}
	}
})

test('Replace sends the confirmed world fingerprint and exposes the saved backup', async () => {
	const { installer, calls, scope, request, conflict } = fixture()
	try {
		const completion = installer.show(request)
		await ready(installer)
		await installer.install()
		await installer.install('replace')
		assert.equal(await completion, 'cf-123-456')
		assert.deepEqual(JSON.parse(JSON.stringify(calls.at(-1)[1].conflict)), conflict)
		assert.equal(installer.backupPath.value, 'backups/world')
	} finally {
		scope.stop()
	}
})

test('an incompatible Minecraft version requires explicit acknowledgement before downloading', async () => {
	const { installer, calls, scope, request } = fixture({ gameVersion: '1.20.1', collision: false })
	try {
		const completion = installer.show(request)
		await ready(installer)
		await installer.install()
		assert.deepEqual(calls, [])
		installer.allowIncompatible.value = true
		await installer.install()
		assert.equal(await completion, 'cf-123-456')
		assert.equal(calls.at(-1)[1].action, 'create')
	} finally {
		scope.stop()
	}
})

test('the shared selector starts a compatible import without showing the old destination form', async () => {
	const f = fixture({ collision: false })
	try {
		const result = await f.installer.show({ ...f.request, autoInstall: true })
		assert.equal(result, 'cf-123-456')
		assert.equal(f.modalShows(), 1) // Only the completed installation is displayed.
		assert.equal(f.calls.at(-1)[1].action, 'create')
	} finally {
		f.scope.stop()
	}
})

test('a missing selected instance never falls back to importing into another instance', async () => {
	const f = fixture({ collision: false })
	try {
		const completion = f.installer.show({ ...f.request, instanceId: 'missing', autoInstall: true })
		await ready(f.installer)
		await f.installer.install()
		assert.equal(f.installer.canInstall.value, false)
		assert.deepEqual(f.calls, [])
		f.installer.cancel()
		assert.equal(await completion, null)
	} finally {
		f.scope.stop()
	}
})
