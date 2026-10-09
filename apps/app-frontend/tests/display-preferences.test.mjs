import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import ts from 'typescript'
import * as vue from 'vue'

function load() {
	const code = ts.transpileModule(
		readFileSync(new URL('../src/helpers/display-preferences.ts', import.meta.url), 'utf8'),
		{ compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } },
	).outputText
	const module = { exports: {} }
	new Function('require', 'module', 'exports', code)(
		(id) => {
			assert.equal(id, 'vue')
			return vue
		},
		module,
		module.exports,
	)
	return module.exports
}

test('explicit host motion settings override the system; other hosts fall back to the system', () => {
	const source = readFileSync(
		new URL('../../../packages/ui/src/utils/motion-preference.ts', import.meta.url),
		'utf8',
	)
	const code = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
	}).outputText
	const module = { exports: {} }
	let systemReduced = false
	const document = { documentElement: { dataset: { reducedMotion: 'off' } } }
	const window = { matchMedia: () => ({ matches: systemReduced }) }
	new Function('module', 'exports', 'document', 'window', code)(
		module,
		module.exports,
		document,
		window,
	)
	const { prefersReducedMotion } = module.exports
	assert.equal(prefersReducedMotion(), false)
	document.documentElement.dataset.reducedMotion = 'on'
	assert.equal(prefersReducedMotion(), true)
	document.documentElement.dataset.reducedMotion = 'off'
	systemReduced = true
	assert.equal(prefersReducedMotion(), false)
	delete document.documentElement.dataset.reducedMotion
	assert.equal(prefersReducedMotion(), true)
})

test('display drafts cancel without writing; saved options survive reopening', () => {
	const { createDisplayPreference } = load()
	let disk = null
	const storage = {
		getItem: () => disk,
		setItem: (_key, value) => {
			disk = value
		},
	}
	const preference = createDisplayPreference(storage)
	const draft = {
		density: 'compact',
		interfaceSize: 'large',
		reduceMotion: true,
	}
	preference.preview.value = draft
	assert.deepEqual(preference.effective.value, draft)
	assert.equal(disk, null)
	preference.preview.value = null
	assert.equal(preference.effective.value.density, 'comfortable')
	preference.set(draft)
	assert.deepEqual(createDisplayPreference(storage).current.value, draft)
})

test('invalid preferences use safe defaults and a failed write leaves the saved values intact', () => {
	const { createDisplayPreference } = load()
	for (const value of [
		'invalid',
		'null',
		'[]',
		'{"density":"compact","interfaceSize":"huge","sidebarCollapsed":"false","reduceMotion":1}',
	]) {
		const preference = createDisplayPreference({
			getItem: () => value,
			setItem() {
				throw new Error('full')
			},
		})
		assert.equal(preference.current.value.interfaceSize, 'normal')
		assert.equal('sidebarCollapsed' in preference.current.value, false)
		assert.equal(preference.current.value.reduceMotion, false)
		assert.throws(
			() => preference.set({ ...preference.current.value, interfaceSize: 'large' }),
			/full/,
		)
		assert.equal(preference.current.value.interfaceSize, 'normal')
	}
})

test('right panel layout is determined by its route', () => {
	const { getRightPanelLayout } = load()
	assert.equal(getRightPanelLayout('/browse/mod'), 'catalog')
	assert.equal(getRightPanelLayout('/project/abc'), 'catalog')
	assert.equal(getRightPanelLayout('/instance/abc'), 'full')
	assert.equal(getRightPanelLayout('/bedrock'), 'full')
	assert.equal(getRightPanelLayout('/unrelated'), 'none')
})

test('runtime previews restore full motion when off and leave icon motion independent', () => {
	const helper = load()
	const dataset = { iconMotion: 'on' }
	const media = {
		matches: true,
		addEventListener() {},
	}
	const code = ts.transpileModule(
		readFileSync(new URL('../src/composables/use-display-preferences.ts', import.meta.url), 'utf8'),
		{ compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } },
	).outputText
	const module = { exports: {} }
	new Function('require', 'module', 'exports', 'window', 'document', 'localStorage', code)(
		(id) => (id === 'vue' ? vue : helper),
		module,
		module.exports,
		{ matchMedia: () => media },
		{ documentElement: { dataset } },
		{ getItem: () => null, setItem() {} },
	)
	const display = module.exports.useDisplayPreferences()
	assert.equal(module.exports.isMotionReduced(), false)
	assert.equal(dataset.interfaceSize, 'normal')
	display.preview.value = {
		...display.current.value,
		density: 'compact',
		interfaceSize: 'large',
		reduceMotion: true,
	}
	assert.equal(dataset.density, 'compact')
	assert.equal(dataset.interfaceSize, 'large')
	assert.equal(module.exports.isMotionReduced(), true)
	assert.equal(dataset.iconMotion, 'on')
	display.preview.value = null
	assert.equal(dataset.interfaceSize, 'normal')
	assert.equal(dataset.reducedMotion, 'off')
	assert.equal(dataset.reducedMotion, 'off')
	assert.equal(dataset.iconMotion, 'on')
})

test('obsolete panel-collapse preferences are ignored when reopening', () => {
	const { createDisplayPreference } = load()
	const preference = createDisplayPreference({
		getItem: () => '{"sidebarCollapsed":true,"interfaceSize":"large"}',
		setItem() {},
	})
	assert.deepEqual(preference.current.value, {
		density: 'comfortable',
		interfaceSize: 'large',
		reduceMotion: false,
	})
})

test('theme changes animate with explicit motion off and stay instant with it on', () => {
	const classes = new Set()
	const document = {
		documentElement: {
			dataset: { reducedMotion: 'off' },
			classList: { remove: (name) => classes.delete(name), add: (name) => classes.add(name) },
			offsetWidth: 100,
		},
	}
	const code = ts.transpileModule(
		readFileSync(
			new URL('../../../packages/ui/src/utils/theme-color-transition.ts', import.meta.url),
			'utf8',
		),
		{ compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } },
	).outputText
	const module = { exports: {} }
	new Function('require', 'module', 'exports', 'document', 'setTimeout', 'clearTimeout', code)(
		() => ({ prefersReducedMotion: () => document.documentElement.dataset.reducedMotion === 'on' }),
		module,
		module.exports,
		document,
		() => 1,
		() => {},
	)
	module.exports.prepareThemeColorTransition()
	assert.equal(classes.has('theme-color-transitioning'), true)
	document.documentElement.dataset.reducedMotion = 'on'
	module.exports.prepareThemeColorTransition()
	assert.equal(classes.has('theme-color-transitioning'), false)
	document.documentElement.dataset.reducedMotion = 'off'
	module.exports.prepareThemeColorTransition()
	assert.equal(classes.has('theme-color-transitioning'), true)
})
