import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import vm from 'node:vm'

import { createDiagnosticExport } from '../src/helpers/diagnostic-export.ts'

test('diagnostic export tracks progress, suppresses duplicate starts and cancels active operation', async () => {
	let resolve,
		calls = 0,
		operation,
		notify
	const states = [],
		cancelled = []
	const controller = createDiagnosticExport({
		save: (id, progress) => {
			calls++
			operation = id
			notify = progress
			return new Promise((r) => {
				resolve = r
			})
		},
		cancel: async (id) => {
			cancelled.push(id)
		},
		changed: (state) => states.push(state),
	})
	const pending = controller.start()
	await controller.start()
	assert.equal(calls, 1)
	notify({ completed: 2, total: 8 })
	assert.deepEqual(states.at(-1), { busy: true, completed: 2, total: 8, saved: false })
	await controller.cancel()
	assert.deepEqual(cancelled, [operation])
	resolve(false)
	await pending
	assert.equal(states.at(-1).busy, false)
	assert.equal(states.at(-1).saved, false)
	await controller.cancel()
	assert.equal(cancelled.length, 1)
})
test('failed export clears busy state and propagates error', async () => {
	const states = []
	const controller = createDiagnosticExport({
		save: async () => {
			throw Error('disk full')
		},
		cancel: async () => {},
		changed: (s) => states.push(s),
	})
	await assert.rejects(controller.start(), /disk full/)
	assert.equal(states.at(-1).busy, false)
})
function webview(origin = 'tauri://localhost') {
	const sends = [],
		listeners = {},
		consoleCalls = []
	let now = 0
	const console = Object.fromEntries(
		['warn', 'error'].map((level) => [level, (...args) => consoleCalls.push(args)]),
	)
	const window = {
		__TAURI_INTERNALS__: {
			invoke: (name, payload) => {
				sends.push(payload)
				return Promise.resolve()
			},
		},
		addEventListener: (name, fn) => {
			listeners[name] = fn
		},
	}
	vm.runInNewContext(
		readFileSync(new URL('../../app/src/api/webview-logs.js', import.meta.url), 'utf8'),
		{ location: { origin }, window, console, Date: { now: () => now }, Error, Promise },
	)
	return {
		sends,
		listeners,
		console,
		consoleCalls,
		advance: () => {
			now += 10001
		},
	}
}
test('WebView logging redacts, bounds and throttles while preserving visible errors', () => {
	const w = webview()
	w.console.error('password=secret Bearer sensitive', {
		get token() {
			throw Error('must not inspect')
		},
	})
	assert.equal(w.sends.length, 1)
	assert(!w.sends[0].message.includes('secret'))
	assert(!w.sends[0].message.includes('sensitive'))
	assert(w.sends[0].message.includes('non-text payload omitted'))
	for (let i = 0; i < 40; i++) w.console.warn('x'.repeat(5000))
	assert.equal(w.sends.length, 20)
	assert.equal(w.consoleCalls.length, 41)
	assert(w.sends.every((p) => p.message.length <= 2048))
	w.advance()
	w.listeners.unhandledrejection({ reason: new Error('Vue rejected') })
	assert.equal(w.sends.length, 21)
})
test('external signin and embedded origins never forward logs', () => {
	const w = webview('https://login.microsoftonline.com')
	w.console.error('credential')
	assert.equal(w.sends.length, 0)
	assert.equal(Object.keys(w.listeners).length, 0)
})
test('confirmation describes archive before saving and offers cancellation', () => {
	const source = readFileSync(
		new URL('../src/components/ui/settings/instances/DiagnosticsExport.vue', import.meta.url),
		'utf8',
	)
	assert(source.includes('ConfirmModalWrapper'))
	assert(source.includes('app.settings.diagnostics.description'))
	assert(source.includes(':disabled="state.busy"'))
	assert(source.includes('@click="cancel"'))
})
test('oversized unlabelled secrets and UTF-8 boundaries are omitted whole', () => {
	const secret = 'custom-value-1234567890'
	for (const input of [
		'x'.repeat(2040) + secret,
		'\u00e9'.repeat(1020) + secret,
		'\u{1f600}'.repeat(510) + secret,
	]) {
		const w = webview()
		w.console.error(input)
		assert.equal(w.sends[0].message, '[oversized payload omitted]')
		assert(!w.sends[0].message.replaceAll(secret, '[REDACTED]').includes('custom-v'))
		assert(Buffer.byteLength(w.sends[0].message) <= 2048)
		assert.equal(w.consoleCalls[0][0], input)
	}
	const w = webview()
	w.console.error('\u00e9'.repeat(1024))
	assert.equal(Buffer.byteLength(w.sends[0].message), 2048)
})
test('Error and arbitrary payload accessors and toJSON are never inspected', () => {
	const w = webview()
	let hits = 0
	const error = new Error('safe')
	for (const key of ['name', 'message', 'stack', 'toJSON'])
		Object.defineProperty(error, key, {
			configurable: true,
			get() {
				hits++
				return 'getter inspected'
			},
		})
	w.console.error(error)
	w.listeners.unhandledrejection({ reason: error })
	assert.equal(hits, 0)
	assert(w.sends.every(({ message }) => message === '[non-text payload omitted]'))
	assert.equal(w.consoleCalls[0][0], error)
})
