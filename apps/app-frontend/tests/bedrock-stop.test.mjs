import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'

import ts from 'typescript'

const source = await readFile(new URL('../src/helpers/bedrock-stop.ts', import.meta.url), 'utf8')
const javascript = ts.transpileModule(source, {
	compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText
const module = { exports: {} }
new Function('module', 'exports', javascript)(module, module.exports)
const { requestBedrockStop } = module.exports

test('a graceful close never asks for confirmation or submits a forced stop', async () => {
	const calls = []
	const result = await requestBedrockStop(
		async (token) => {
			calls.push(token)
			return { state: 'stopped', force_token: null }
		},
		async () => {
			assert.fail('graceful close should not prompt')
		},
	)
	assert.equal(result.state, 'stopped')
	assert.deepEqual(calls, [undefined])
})

test('canceling after a graceful timeout keeps the game running', async () => {
	const calls = []
	const result = await requestBedrockStop(
		async (token) => {
			calls.push(token)
			return { state: 'timeout', force_token: 'retained-process' }
		},
		async () => false,
	)
	assert.equal(result.state, 'timeout')
	assert.deepEqual(calls, [undefined])
})

test('only explicit confirmation sends the token for the same retained process', async () => {
	const events = []
	await requestBedrockStop(
		async (token) => {
			events.push(token ?? 'normal')
			return token
				? { state: 'stopped', force_token: null }
				: { state: 'timeout', force_token: 'exact-process-token' }
		},
		async () => {
			events.push('confirmed')
			return true
		},
	)
	assert.deepEqual(events, ['normal', 'confirmed', 'exact-process-token'])
})

test('missing tokens and failed normal requests never offer forced termination', async () => {
	const prompt = async () => {
		assert.fail('must not prompt')
	}
	assert.equal(
		(await requestBedrockStop(async () => ({ state: 'timeout', force_token: null }), prompt)).state,
		'timeout',
	)
	await assert.rejects(
		requestBedrockStop(async () => {
			throw new Error('identity failed')
		}, prompt),
		/identity failed/,
	)
})

test('a dismissed or failed confirmation never sends a forced request', async () => {
	const calls = []
	await assert.rejects(
		requestBedrockStop(
			async (token) => {
				calls.push(token)
				return { state: 'timeout', force_token: 'retained-process' }
			},
			async () => {
				throw new Error('modal unavailable')
			},
		),
		/modal unavailable/,
	)
	assert.deepEqual(calls, [undefined])
})
