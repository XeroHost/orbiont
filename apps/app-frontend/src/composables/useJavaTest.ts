import { onScopeDispose, ref } from 'vue'

import { get_jre, test_jre } from '@/helpers/jre.js'

export default function useJavaTest() {
	const testingJava = ref(false)
	const javaTestResult = ref<boolean | null>(null)
	const javaCompatibilityResult = ref<boolean | null>(null)
	let testDebounceTimer: ReturnType<typeof setTimeout> | null = null
	let requestId = 0
	async function runJavaTest(path: string, version: number | null) {
		const request = ++requestId
		if (testDebounceTimer) {
			clearTimeout(testDebounceTimer)
			testDebounceTimer = null
		}
		javaTestResult.value = null
		javaCompatibilityResult.value = null
		if (!path) {
			testingJava.value = false
			return
		}
		testingJava.value = true
		try {
			const valid = !!(await get_jre(path))
			const compatible = valid && version !== null ? await test_jre(path, version) : null
			if (request !== requestId) return
			javaTestResult.value = valid
			javaCompatibilityResult.value = compatible
		} catch {
			if (request === requestId) javaTestResult.value = false
		} finally {
			if (request === requestId) testingJava.value = false
		}
	}
	function testJavaInstallationDebounced(path: string, version: number | null, delay = 600) {
		++requestId
		if (testDebounceTimer) clearTimeout(testDebounceTimer)
		javaTestResult.value = null
		javaCompatibilityResult.value = null
		testingJava.value = false
		if (!path) return
		testDebounceTimer = setTimeout(() => runJavaTest(path, version), delay)
	}
	onScopeDispose(() => {
		++requestId
		if (testDebounceTimer) clearTimeout(testDebounceTimer)
	})
	return {
		testingJava,
		javaTestResult,
		javaCompatibilityResult,
		testJavaInstallationDebounced,
		testJavaInstallation: runJavaTest,
	}
}
