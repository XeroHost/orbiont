import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import ts from 'typescript'

test('resize delivery is deferred, batched, and cancelled when disconnected', () => {
	const source = readFileSync(
		new URL('../../../packages/ui/src/utils/resize-observer.ts', import.meta.url),
		'utf8',
	)
	const code = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
	}).outputText
	let deliver
	let disconnected = false
	const frames = new Map()
	let nextFrame = 0
	class ResizeObserver {
		constructor(callback) {
			deliver = callback
		}
		observe() {}
		unobserve() {}
		disconnect() {
			disconnected = true
		}
	}
	const module = { exports: {} }
	new Function(
		'module',
		'exports',
		'ResizeObserver',
		'requestAnimationFrame',
		'cancelAnimationFrame',
		code,
	)(
		module,
		module.exports,
		ResizeObserver,
		(callback) => {
			frames.set(++nextFrame, callback)
			return nextFrame
		},
		(id) => frames.delete(id),
	)
	const seen = []
	const observer = module.exports.createFrameResizeObserver((entries) => seen.push(entries))
	const target = {}
	deliver([{ target, contentRect: { width: 100 } }])
	deliver([{ target, contentRect: { width: 200 } }])
	assert.equal(seen.length, 0, 'no layout write during observer delivery')
	assert.equal(frames.size, 1)
	frames.get(1)()
	frames.delete(1)
	assert.equal(seen[0][0].contentRect.width, 200)
	deliver([{ target, contentRect: { width: 300 } }])
	observer.disconnect()
	assert.equal(frames.size, 0)
	assert.equal(disconnected, true)
	assert.equal(seen.length, 1)
})
