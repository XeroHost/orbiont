/** Coalesce navigation/close requests so a second request cannot replace the modal resolver. */
export function createDraftExit(isDirty: () => boolean, prompt: () => Promise<boolean>) {
	let pending: Promise<boolean> | undefined
	return {
		confirm(): Promise<boolean> {
			if (pending) return pending
			if (!isDirty()) return Promise.resolve(true)
			pending = prompt()
				.catch(() => false)
				.finally(() => {
					pending = undefined
				})
			return pending
		},
	}
}
