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
const { createAccentPreference } = load(
	readFileSync(new URL('../src/helpers/accent.ts', import.meta.url), 'utf8'),
	{ vue },
)
const { createDisplayPreference } = load(
	readFileSync(new URL('../src/helpers/display-preferences.ts', import.meta.url), 'utf8'),
	{ vue },
)
const { useSavable } = load(
	readFileSync(new URL('../../../packages/ui/src/utils/savable.ts', import.meta.url), 'utf8'),
	{ vue, 'es-toolkit': require('es-toolkit') },
)
const script = compileScript(
	parse(
		readFileSync(
			new URL('../src/components/ui/settings/display/AppearanceSettings.vue', import.meta.url),
			'utf8',
		),
	).descriptor,
	{ id: 'appearance-accent' },
).content

function fixture() {
	let stored = 'cyan',
		controller,
		fail = false
	const accent = createAccentPreference({
		getItem: () => stored,
		setItem: (_key, value) => {
			stored = value
		},
	})
	let displayDisk = null
	const display = createDisplayPreference({
		getItem: () => displayDisk,
		setItem: (_key, value) => {
			displayDisk = value
		},
	})
	const theme = vue.reactive({
		preferred: 'dark',
		preferredDark: 'dark',
		active: 'dark',
		advancedRendering: true,
		preview: null,
		options: ['dark', 'light', 'oled', 'system'],
	})
	const settings = { nativeDecorations: false, devMode: false }
	const iconMotion = {
		current: vue.ref('on'),
		preview: vue.ref(null),
		set(value) {
			this.current.value = value
		},
	}
	const contextKey = Symbol()
	const component = load(script, {
		vue,
		'@orbiont/ui': {
			useSavable,
			defineMessages: (value) => value,
			useVIntl: () => ({ formatMessage: (message) => message.defaultMessage }),
			provideAppearanceSettings() {},
		},
		'@tauri-apps/plugin-os': { platform: () => 'windows' },
		'@/composables/use-app-settings.ts': { useAppSettings: () => settings },
		'@/composables/use-accent': { useAccent: () => accent },
		'@/composables/use-display-preferences': { useDisplayPreferences: () => display },
		'@/composables/use-icon-motion': { useIconMotion: () => iconMotion },
		'@/composables/use-theme.ts': {
			useTheme: () => theme,
			isDarkTheme: (value) => ['dark', 'oled', 'retro'].includes(value),
		},
		'./AccentSettings.vue': {},
		'./LayoutSettings.vue': {},
		'@/helpers/settings.ts': {
			get: async () => ({}),
			set: async () => {
				if (fail) throw new Error('native write failed')
			},
		},
		'@/providers/app-settings-modal': { appSettingsModalContextKey: contextKey },
	}).default
	let state
	const app = vue
		.createRenderer({
			createComment: () => ({}),
			insert() {},
			remove() {},
			parentNode: () => null,
			nextSibling: () => null,
		})
		.createApp({
			...component,
			setup(props, context) {
				state = component.setup(props, context)
				return () => null
			},
		})
	app.provide(contextKey, {
		registerUnsavedChangesController(value) {
			controller = value
		},
	})
	app.mount({})
	return {
		state,
		accent,
		display,
		theme,
		controller: () => controller,
		stored: () => stored,
		fail: () => {
			fail = true
		},
		dispose: () => app.unmount(),
	}
}

test('the real appearance form previews, discards, saves and resets accent with its shared action bar', async () => {
	const view = fixture()
	try {
		view.state.current.value.accent = 'blue'
		assert.equal(view.accent.effective.value, 'blue')
		assert.equal(view.stored(), 'cyan')
		assert.equal(view.controller().hasChanges(), true)
		view.controller().reset()
		assert.equal(view.accent.effective.value, 'cyan')
		view.state.current.value.accent = 'violet'
		await view.controller().save()
		assert.equal(view.stored(), 'violet')
		assert.equal(view.controller().hasChanges(), false)
		view.state.current.value.accent = 'cyan'
		await view.controller().save()
		assert.equal(view.stored(), 'cyan')
		view.state.current.value.accent = 'pink'
	} finally {
		view.dispose()
	}
	assert.equal(view.accent.preview.value, null)
	assert.equal(view.accent.effective.value, 'cyan')
})

test('the shared form previews density, size and reduced motion, cancels and saves them together', async () => {
	const view = fixture()
	const draft = {
		density: 'compact',
		interfaceSize: 'large',
		reduceMotion: true,
	}
	try {
		view.state.current.value.display = draft
		assert.deepEqual(view.display.effective.value, draft)
		assert.equal(view.display.current.value.density, 'comfortable')
		view.controller().reset()
		assert.equal(view.display.effective.value.interfaceSize, 'normal')
		view.state.current.value.display = draft
		await view.controller().save()
		assert.deepEqual(view.display.current.value, draft)
		assert.equal(view.controller().hasChanges(), false)
		view.state.current.value.display.interfaceSize = 'small'
	} finally {
		view.dispose()
	}
	assert.equal(view.display.preview.value, null)
	assert.equal(view.display.effective.value.interfaceSize, 'large')
})

test('a failed settings save keeps the accent draft and reports the failure', async () => {
	const view = fixture()
	try {
		view.state.current.value.accent = 'amber'
		view.fail()
		await assert.rejects(view.controller().save(), /native write failed/)
		assert.equal(view.stored(), 'cyan')
		assert.equal(view.controller().hasChanges(), true)
		assert.equal(view.accent.effective.value, 'amber')
	} finally {
		view.dispose()
	}
})
