/** Cache concurrent loads and successful results; allow another attempt after failure. */
export function createLazyLoader<T>(load: () => Promise<T>) {
	let pending: Promise<T> | undefined
	return () => {
		pending ??= load().catch((error: unknown) => {
			pending = undefined
			throw error
		})
		return pending
	}
}
