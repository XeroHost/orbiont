import { spawnSync } from 'node:child_process'
import { resolve } from 'node:path'
import test from 'node:test'
import assert from 'node:assert/strict'

test('resolved native tasks retain offline/stack/nextest/cache environments without hashing runtime credentials', () => {
	const env = {
		...process.env,
		SQLX_OFFLINE: 'true',
		RUST_MIN_STACK: '134217728',
		NEXTEST_NO_TESTS: 'pass',
		RUSTC_WRAPPER: 'sccache',
		SCCACHE_GHA_ENABLED: 'on',
		ACTIONS_CACHE_SERVICE_V2: 'true',
		ACTIONS_RESULTS_URL: 'https://ci-sentinel.invalid',
		ACTIONS_RUNTIME_TOKEN: 'ci-sentinel-not-a-real-token',
	}
	function dry(token) {
		const result = spawnSync(
			process.execPath,
			[
				resolve('node_modules/turbo/bin/turbo'),
				'run',
				'lint',
				'test',
				'build',
				'--filter=@orbiont/app',
				'--filter=@orbiont/app-lib',
				'--filter=@orbiont/daedalus',
				'--dry=json',
			],
			{ encoding: 'utf8', env: { ...env, ACTIONS_RUNTIME_TOKEN: token } },
		)
		assert.equal(result.status, 0, result.stderr)
		return JSON.parse(result.stdout).tasks.filter((task) =>
			/^@orbiont\/(app|app-lib|daedalus)#/.test(task.taskId),
		)
	}
	const first = dry(env.ACTIONS_RUNTIME_TOKEN),
		changed = dry('ci-another-sentinel-not-a-real-token')
	assert.equal(first.length, 9)
	for (const task of first) {
		assert.equal(task.resolvedTaskDefinition.cache, false, task.taskId)
		for (const name of ['SQLX_OFFLINE', 'RUST_MIN_STACK', 'NEXTEST_NO_TESTS'])
			assert.ok(
				task.environmentVariables.configured.some((value) => value.startsWith(`${name}=`)),
				`${task.taskId}: ${name}`,
			)
		for (const name of [
			'RUSTC_WRAPPER',
			'SCCACHE_GHA_ENABLED',
			'ACTIONS_CACHE_SERVICE_V2',
			'ACTIONS_RESULTS_URL',
			'ACTIONS_RUNTIME_TOKEN',
		])
			assert.ok(
				task.environmentVariables.passthrough.some((value) => value.startsWith(`${name}=`)),
				`${task.taskId}: ${name}`,
			)
		assert.ok(
			!task.environmentVariables.configured.some((value) =>
				value.startsWith('ACTIONS_RUNTIME_TOKEN='),
			),
		)
		assert.equal(
			task.hash,
			changed.find((other) => other.taskId === task.taskId).hash,
			`${task.taskId}: runtime token must not affect task hash`,
		)
	}
})
