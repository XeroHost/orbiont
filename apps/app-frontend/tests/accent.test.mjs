import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import ts from 'typescript'
import * as vue from 'vue'

const code = ts.transpileModule(
	readFileSync(new URL('../src/helpers/accent.ts', import.meta.url), 'utf8'),
	{ compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } },
).outputText
const module = { exports: {} }
new Function('require', 'module', 'exports', code)(
	(name) => {
		assert.equal(name, 'vue')
		return vue
	},
	module,
	module.exports,
)
const { createAccentPreference, getAccentPalette, ACCENT_OPTIONS } = module.exports

function storage(initial) {
	const values = new Map(initial ? [['orbiont.accent', initial]] : [])
	return {
		getItem: (key) => values.get(key) ?? null,
		setItem: (key, value) => values.set(key, value),
	}
}

test('preview and cancellation preserve the saved accent; saving survives reopening', () => {
	const disk = storage('blue')
	const preference = createAccentPreference(disk)
	assert.equal(preference.current.value, 'blue')
	preference.preview.value = 'violet'
	assert.equal(preference.effective.value, 'violet')
	assert.equal(createAccentPreference(disk).current.value, 'blue')
	preference.preview.value = null
	assert.equal(preference.effective.value, 'blue')
	preference.set('emerald')
	assert.equal(createAccentPreference(disk).current.value, 'emerald')
	preference.set('cyan')
	assert.equal(createAccentPreference(disk).current.value, 'cyan')
})

test('invalid or inaccessible saved values fall back; failed writes do not claim a saved preference', () => {
	assert.equal(createAccentPreference(storage('invalid')).current.value, 'cyan')
	const preference = createAccentPreference({
		getItem() {
			throw new Error('blocked')
		},
		setItem() {
			throw new Error('full')
		},
	})
	assert.equal(preference.current.value, 'cyan')
	assert.throws(() => preference.set('blue'), /full/)
	assert.equal(preference.current.value, 'cyan')
})

function luminance(hex) {
	const rgb = hex.match(/[0-9a-f]{2}/gi).map((channel) => {
		const value = parseInt(channel, 16) / 255
		return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
	})
	return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722
}
function contrast(a, b) {
	const values = [luminance(a), luminance(b)].sort((x, y) => y - x)
	return (values[0] + 0.05) / (values[1] + 0.05)
}

test('all fifteen accents retain readable button text and visible focus in light, dark and OLED', () => {
	assert.equal(ACCENT_OPTIONS.length, 15)
	for (const accent of ACCENT_OPTIONS) {
		for (const mode of ['light', 'dark', 'oled', 'retro']) {
			const { brand, focus } = getAccentPalette(accent, mode)
			const lightButtons = mode === 'light' || mode === 'retro'
			assert.ok(
				contrast(brand, lightButtons ? '#ffffff' : '#000000') >= 4.5,
				`${accent}/${mode}: text`,
			)
			assert.ok(
				contrast(focus, mode === 'light' ? '#f8f8f8' : '#27292e') >= 3,
				`${accent}/${mode}: focus`,
			)
		}
	}
})
