import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { parse } from 'yaml'
import { verifyGate } from './ci-release-gate.mjs'
import { verifyUniversalInputs } from './ci-universal-inputs.mjs'
const read = (path) => readFileSync(path, 'utf8')
const build = parse(read('.github/workflows/theseus-build.yml'))
const ci = parse(read('.github/workflows/turbo-ci.yml'))
const release = parse(read('.github/workflows/theseus-release.yml'))
const turbo = JSON.parse(read('turbo.jsonc'))
const gate = {
	validation: 'success',
	build: 'success',
	sha: 'a'.repeat(40),
	ref: 'refs/tags/v1.0.0',
}
test('release requires validation and every installer for the same immutable source', () => {
	assert.deepEqual(verifyGate(gate, { sha: gate.sha, ref: gate.ref }), gate)
	for (const status of ['failure', 'cancelled', 'skipped', ''])
		for (const key of ['validation', 'build'])
			assert.throws(() => verifyGate({ ...gate, [key]: status }))
	assert.throws(() => verifyGate(gate, { sha: 'b'.repeat(40), ref: gate.ref }))
	assert.throws(() => verifyGate(gate, { sha: gate.sha, ref: 'refs/tags/v2' }))
	assert.deepEqual(build.jobs['release-gate'].needs, ['source', 'validation', 'build'])
	assert.ok(build.jobs['release-gate'].if.includes('always()'))
	assert.ok(
		release.jobs.release.steps.some((step) => step.run?.includes('ci-release-gate.mjs --verify')),
	)
	assert.ok(!build.jobs.build.needs.includes('validation'))
})
test('trusted cache saves exclude PRs and stores cannot include secrets or compiled targets', () => {
	for (const workflow of [build, ci])
		for (const job of Object.values(workflow.jobs))
			for (const step of job.steps || []) {
				if (step.uses?.includes('cache/save@')) {
					assert.ok(step.if.includes("github.event_name != 'pull_request'"))
					assert.ok(step.if.includes('refs/heads/main'))
					assert.ok(!/(target|\.env|node_modules)/.test(step.with.path))
				}
				if (step.uses?.includes('cache/restore@')) {
					assert.ok(step.with.key.includes('runner.os'))
					assert.ok(step.with.key.includes('runner.arch'))
				}
			}
	assert.ok(
		build.jobs.source.steps.some((step) => step.run?.includes('git tag --points-at "$GITHUB_SHA"')),
	)
})
test('Rust workspace, patch, lock, root config and Java changes invalidate task inputs', () => {
	for (const path of [
		'Cargo.toml',
		'Cargo.lock',
		'rust-toolchain.toml',
		'.cargo/**',
		'patches/**',
		'apps/app/**',
		'packages/app-lib/**',
		'packages/content-management/**',
		'packages/daedalus/**',
		'packages/path-util/**',
		'packages/serde-binhum/**',
		'packages/async-minecraft-ping/**',
	])
		assert.ok(turbo.globalDependencies.includes(path), path)
	for (const name of ['app', 'app-lib', 'daedalus'])
		for (const task of ['build', 'lint', 'test'])
			assert.equal(turbo.tasks[`@orbiont/${name}#${task}`].cache, false)
	assert.ok(turbo.globalEnv.includes('JAVA_HOME'))
})
test('security, Bedrock and GLib jobs survive; Cargo runs serially', () => {
	for (const job of [
		'dependency-security',
		'windows-bedrock',
		'glib-backport',
		'native-platform-fixtures',
		'javascript',
	])
		assert.ok(ci.jobs[job])
	const pkg = JSON.parse(read('package.json'))
	assert.ok(pkg.scripts['ci:native'].includes('--concurrency=1'))
	for (const name of ['app', 'app-lib', 'daedalus'])
		assert.ok(pkg.scripts['ci:js'].includes(`--filter=!@orbiont/${name}`))
})
test('universal collection requires both unsigned architectures with identical build inputs', () => {
	const expected = {
		sha: gate.sha,
		configHash: 'config',
		frontendHash: 'frontend',
		features: 'updater',
		profile: 'release',
	}
	const entries = ['aarch64-apple-darwin', 'x86_64-apple-darwin'].map((target) => ({
		...expected,
		target,
		signed: false,
	}))
	assert.equal(verifyUniversalInputs(entries, expected).length, 2)
	assert.throws(() => verifyUniversalInputs(entries.slice(0, 1), expected))
	assert.throws(() => verifyUniversalInputs([entries[0], entries[0]], expected))
	for (const key of ['sha', 'configHash', 'frontendHash', 'features', 'profile'])
		assert.throws(() =>
			verifyUniversalInputs([{ ...entries[0], [key]: 'different' }, entries[1]], expected),
		)
	assert.throws(() =>
		verifyUniversalInputs([{ ...entries[0], signed: true }, entries[1]], expected),
	)
})

test('compiler cache explicitly separates PR writes from trusted main/tag readers', () => {
	for (const workflow of [build, ci])
		for (const job of Object.values(workflow.jobs))
			for (const step of job.steps || []) {
				if (step.name !== 'Configure compiler cache scope') continue
				assert.ok(step.run.includes("github.event_name == 'pull_request'"))
				assert.ok(
					step.run.includes("format('pr-{0}', github.event.pull_request.number) || 'trusted'"),
				)
				assert.ok(step.run.includes('SCCACHE_GHA_ENABLED=on'))
				if (workflow === ci) assert.ok(!step.run.includes('matrix.artifact-target-name'))
			}
})
