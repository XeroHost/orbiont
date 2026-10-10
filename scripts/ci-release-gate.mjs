import { writeFileSync, readFileSync } from 'node:fs'
export function verifyGate({ validation, build, sha, ref }, expected) {
	if (validation !== 'success' || build !== 'success')
		throw new Error('Validation and all installer builds must succeed')
	if (!/^[0-9a-f]{40}$/.test(sha) || !/^refs\/(heads|tags)\//.test(ref))
		throw new Error('Invalid immutable source identity')
	if (expected && (sha !== expected.sha || ref !== expected.ref))
		throw new Error('Release source does not match successful build')
	return { validation, build, sha, ref }
}
if (process.argv[1]?.endsWith('ci-release-gate.mjs')) {
	if (process.argv[2] === '--verify') {
		verifyGate(JSON.parse(readFileSync('Release gate/release-gate.json', 'utf8')), {
			sha: process.env.HEAD_SHA,
			ref: `refs/tags/${process.env.VERSION_TAG}`,
		})
	} else {
		const gate = verifyGate({
			validation: process.env.VALIDATION_RESULT,
			build: process.env.BUILD_RESULT,
			sha: process.env.SOURCE_SHA,
			ref: process.env.SOURCE_REF,
		})
		writeFileSync('release-gate.json', JSON.stringify(gate, null, 2))
	}
}
