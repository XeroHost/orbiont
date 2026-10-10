import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { test } from 'node:test'

import * as vue from 'vue'
const ts = createRequire(import.meta.url)('typescript')

test('Windows AltGr avoids synthetic Control and recording releases all listeners', () => {
	const source = readFileSync(
		new URL(
			'../src/components/ui/settings/instances/game-settings-modal/keybind-input.vue',
			import.meta.url,
		),
		'utf8',
	).match(/<script setup lang="ts">([\s\S]*?)<\/script>/)[1]
	const code = ts.transpileModule(
		source +
			'\nexports.controls = { startRecording, cancelRecording, handleKeydown, handleKeyup };',
		{ compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } },
	).outputText
	const listeners = new Map(),
		emitted = [],
		exports = {}
	let unmount
	const imports = {
		'@orbiont/assets': {},
		'@orbiont/ui': {
			defineMessages: (value) => value,
			useVIntl: () => ({
				formatMessage: (value) => value.defaultMessage,
				locale: vue.ref('en-US'),
			}),
		},
		vue: {
			...vue,
			useId: () => 'test',
			onBeforeUnmount: (callback) => {
				unmount = callback
			},
		},
		'./keybinds': {
			activateKeybindRecording() {},
			deactivateKeybindRecording() {},
			formatMinecraftKeybind: (_, key) => key,
			minecraftKeyTokenFromKeyboardEvent: (event) =>
				({
					ControlLeft: 'key.keyboard.left.control',
					AltRight: 'key.keyboard.right.alt',
					KeyA: 'key.keyboard.a',
				})[event.code],
		},
	}
	const window = {
		addEventListener: (name, fn) => listeners.set(name, fn),
		removeEventListener: (name) => listeners.delete(name),
	}
	new Function(
		'require',
		'exports',
		'defineProps',
		'defineEmits',
		'withDefaults',
		'navigator',
		'window',
		code,
	)(
		(name) => imports[name],
		exports,
		() => ({ modelValue: '', settingLabel: 'Test', conflicts: [] }),
		() => (_, value) => emitted.push(value),
		(value) => value,
		{ platform: 'Win32' },
		window,
	)
	const controls = exports.controls
	const event = (code) => ({ code, preventDefault() {}, stopPropagation() {}, repeat: false })
	controls.startRecording()
	assert.equal(listeners.size, 4)
	controls.handleKeydown(event('ControlLeft'))
	assert.deepEqual(emitted, [])
	controls.handleKeydown(event('AltRight'))
	assert.deepEqual(emitted, ['key.keyboard.right.alt'])
	assert.equal(listeners.size, 0)
	controls.startRecording()
	controls.handleKeydown(event('ControlLeft'))
	controls.handleKeyup(event('ControlLeft'))
	assert.deepEqual(emitted, ['key.keyboard.right.alt', 'key.keyboard.left.control'])
	controls.startRecording()
	controls.handleKeydown(event('ControlLeft'))
	controls.cancelRecording()
	assert.equal(listeners.size, 0)
	controls.startRecording()
	controls.handleKeydown(event('KeyA'))
	assert.equal(emitted.at(-1), 'key.keyboard.a')
	controls.startRecording()
	unmount()
	assert.equal(listeners.size, 0)
})
