import { ref } from 'vue'

import type { UpdateAllSelection } from './update-all-modal-types'

export function useUpdateAllValidation(
	validate: () => ((selections: UpdateAllSelection[]) => Promise<void>) | undefined,
) {
	const validating = ref(false)
	const validationError = ref<string | null>(null)
	let request = 0
	function reset() {
		++request
		validating.value = false
		validationError.value = null
	}
	async function run(selections: UpdateAllSelection[]) {
		const current = ++request
		validating.value = true
		validationError.value = null
		try {
			await validate()?.(selections)
			return current === request
		} catch (error) {
			if (current === request)
				validationError.value = error instanceof Error ? error.message : String(error)
			return false
		} finally {
			if (current === request) validating.value = false
		}
	}
	return { validating, validationError, run, reset }
}
