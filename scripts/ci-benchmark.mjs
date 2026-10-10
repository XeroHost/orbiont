import { spawnSync } from 'node:child_process'
import { writeFileSync, statSync, existsSync } from 'node:fs'
import { performance } from 'node:perf_hooks'
// Opt-in, serial and isolated. Environment overrides leave profile.release unchanged.
const mode = process.argv[2] || 'tasks'
const results = []
function run(label, command, args, env = {}) {
	const start = performance.now()
	const result = spawnSync(command, args, {
		stdio: 'inherit',
		env: { ...process.env, ...env },
		shell: process.platform === 'win32',
	})
	results.push({
		label,
		seconds: (performance.now() - start) / 1000,
		status: result.status,
		error: result.error?.message,
	})
	writeFileSync('ci-benchmark.json', JSON.stringify({ mode, results }, null, 2))
	if (result.status !== 0) process.exit(result.status || 1)
}
if (mode === 'tasks') {
	for (const pass of ['first', 'repeat'])
		for (const task of ['ci:js', 'ci:native']) run(`${pass}-${task}`, 'pnpm', [task])
} else if (mode === 'release-lto') {
	for (const lto of ['fat', 'thin']) {
		const target = `target/benchmark-${lto}`
		run(lto, 'cargo', ['build', '--locked', '--release', '-p', 'theseus_gui'], {
			CARGO_PROFILE_RELEASE_LTO: lto,
			CARGO_TARGET_DIR: target,
		})
		const binary = `${target}/release/${process.platform === 'win32' ? 'theseus_gui.exe' : 'theseus_gui'}`
		results.at(-1).bytes = existsSync(binary) ? statSync(binary).size : null
		results.at(-1).limitation =
			'Unsigned default-feature binary benchmark; installer/startup/updater checks require platform packaging. Frontend dist and fixture .env must be prepared first.'
		writeFileSync('ci-benchmark.json', JSON.stringify({ mode, results }, null, 2))
	}
} else throw new Error('Expected tasks or release-lto')
