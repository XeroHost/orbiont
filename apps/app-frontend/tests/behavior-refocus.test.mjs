import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { test } from 'node:test'

import ts from 'typescript'
import * as vue from 'vue'
import { compileScript, parse } from 'vue/compiler-sfc'

const require = createRequire(new URL('../../../packages/ui/package.json', import.meta.url))
function load(source, imports) {
	const code = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
	}).outputText
	const module = { exports: {} }
	new Function('require', 'module', 'exports', code)(
		(id) => {
			if (id in imports) return imports[id]
			throw new Error(`Unexpected import ${id}`)
		},
		module,
		module.exports,
	)
	return module.exports
}
const appSettingsModule = load(
	readFileSync(new URL('../src/composables/use-app-settings.ts', import.meta.url), 'utf8'),
	{ vue },
)
const { useSavable } = load(
	readFileSync(new URL('../../../packages/ui/src/utils/savable.ts', import.meta.url), 'utf8'),
	{ vue, 'es-toolkit': require('es-toolkit') },
)
const script = compileScript(
	parse(
		readFileSync(
			new URL('../src/components/ui/settings/display/BehaviorSettings.vue', import.meta.url),
			'utf8',
		),
	).descriptor,
	{ id: 'behavior-refocus' },
).content

async function fixture() {
	Object.assign(
		appSettingsModule.useAppSettings().featureFlags,
		appSettingsModule.DEFAULT_FEATURE_FLAGS,
	)
	let disk = {
		hide_on_process_start: false,
		hide_nametag_skins_page: false,
		feature_flags: { worlds_in_home: false },
		locale: 'es-419',
	}
	let writes = 0,
		fail = false,
		controller,
		state
	const queryData = vue.ref(structuredClone(disk))
	const contextKey = Symbol()
	const queryClient = {
		setQueryData(_key, value) {
			queryData.value = value
		},
		cancelQueries: async () => {},
		invalidateQueries: async () => {},
	}
	const component = load(script, {
		vue,
		'@orbiont/ui': {
			useSavable,
			defineMessages: (value) => value,
			useVIntl: () => ({ formatMessage: (message) => message.defaultMessage }),
			injectNotificationManager: () => ({ handleError() {} }),
		},
		'@tanstack/vue-query': {
			useQuery: () => ({ data: queryData, suspense: async () => {} }),
			useQueryClient: () => queryClient,
			useMutation: (options) => ({ mutateAsync: async (value) => options.mutationFn(value) }),
		},
		'@/composables/use-app-settings.ts': appSettingsModule,
		'@/helpers/settings.ts': {
			appSettingsKeys: { all: ['app-settings'], update: ['app-settings', 'update'] },
			appSettingsQueryOptions: () => ({}),
			get: async () => structuredClone(disk),
			set: async (value) => {
				if (fail) throw Error('save failed')
				writes++
				disk = structuredClone(value)
			},
		},
		'@/providers/app-settings-modal': { appSettingsModalContextKey: contextKey },
	}).default
	const renderer = vue.createRenderer({
		createComment: () => ({}),
		createElement: () => ({}),
		insert() {},
		remove() {},
		parentNode: () => null,
		nextSibling: () => null,
	})
	const app = renderer.createApp({
		render: () =>
			vue.h(vue.Suspense, null, {
				default: () =>
					vue.h({
						...component,
						async setup(props, context) {
							state = await component.setup(props, context)
							return () => null
						},
					}),
			}),
	})
	app.provide(contextKey, {
		registerUnsavedChangesController: (value) => {
			controller = value
		},
	})
	app.mount({})
	for (let i = 0; i < 5; i++) await vue.nextTick()
	return {
		state,
		controller: () => controller,
		disk: () => disk,
		writes: () => writes,
		fail: () => {
			fail = true
		},
		dispose: () => app.unmount(),
	}
}

test('refocus drafts cancel and save independently of minimizing; other settings survive', async () => {
	const view = await fixture()
	try {
		assert.equal(view.state.current.value.refocusApp, true)
		view.state.current.value.refocusApp = false
		view.controller().reset()
		assert.equal(view.state.current.value.refocusApp, true)
		assert.equal(view.writes(), 0)
		view.state.current.value.refocusApp = false
		await view.controller().save()
		assert.equal(view.disk().feature_flags.refocus_on_game_exit, false)
		assert.equal(view.disk().hide_on_process_start, false)
		assert.equal(view.disk().feature_flags.worlds_in_home, false)
		assert.equal(view.disk().locale, 'es-419')
		assert.equal(view.controller().hasChanges(), false)
	} finally {
		view.dispose()
	}
})

test('failed refocus saves keep the draft and do not update the runtime preference', async () => {
	const view = await fixture()
	try {
		view.state.current.value.refocusApp = false
		view.fail()
		await view.controller().save()
		assert.equal(view.controller().hasChanges(), true)
		assert.equal(view.writes(), 0)
		assert.equal(view.disk().hide_on_process_start, false)
		assert.equal(appSettingsModule.useAppSettings().featureFlags.refocus_on_game_exit, true)
	} finally {
		view.dispose()
	}
})
