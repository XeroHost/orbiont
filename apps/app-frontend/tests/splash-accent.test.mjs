import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import ts from 'typescript'
import * as vue from 'vue'

function load(path, imports) {
	const source = readFileSync(new URL(path, import.meta.url), 'utf8')
	const code = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
	}).outputText
	const module = { exports: {} }
	new Function('require', 'module', 'exports', code)(
		(name) => {
			assert.ok(name in imports, `Unexpected import ${name}`)
			return imports[name]
		},
		module,
		module.exports,
	)
	return module.exports
}
const accents = load('../src/helpers/accent.ts', { vue })
const { getSplashAccentMatrix } = load('../src/helpers/splash-accent.ts', {
	'@/helpers/accent': accents,
})
function transform(matrix, pixel) {
	const values = matrix.split(' ').map(Number)
	assert.equal(values.length, 20)
	assert.ok(values.every(Number.isFinite))
	return Array.from({ length: 4 }, (_, row) =>
		pixel.reduce(
			(sum, value, column) => sum + value * values[row * 5 + column],
			values[row * 5 + 4],
		),
	)
}
function close(actual, expected) {
	actual.forEach((value, index) => assert.ok(Math.abs(value - expected[index]) < 1e-9))
}

test('default cyan bypasses the filter to preserve the original artwork exactly', () => {
	assert.equal(getSplashAccentMatrix('cyan'), null)
})

test('every accent recolors cyan into its dark palette hue, preserving shading and alpha', () => {
	for (const accent of accents.ACCENT_OPTIONS.filter((value) => value !== 'cyan')) {
		const matrix = getSplashAccentMatrix(accent)
		const rgb = accents
			.getAccentPalette(accent, 'dark')
			.brand.match(/[0-9a-f]{2}/gi)
			.map((v) => parseInt(v, 16))
		const maximum = Math.max(...rgb)
		for (const intensity of [0.1, 0.5, 1]) {
			close(transform(matrix, [0, intensity, intensity, 0.65]), [
				...rgb.map((v) => (intensity * v) / maximum),
				0.65,
			])
		}
	}
})

test('neutral blacks, grays and white highlights are preserved for all accents', () => {
	for (const accent of accents.ACCENT_OPTIONS.filter((value) => value !== 'cyan')) {
		for (const gray of [0, 0.05, 0.5, 1]) {
			close(transform(getSplashAccentMatrix(accent), [gray, gray, gray, 1]), [gray, gray, gray, 1])
		}
	}
})
