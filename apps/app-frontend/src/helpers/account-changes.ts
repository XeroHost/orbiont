const listeners = new Set<() => void>()

/** Successful account mutations notify consumers without recurring native reads. */
export function notifyAccountChange() {
	for (const listener of listeners) listener()
}

export function onAccountChange(listener: () => void) {
	listeners.add(listener)
	return () => listeners.delete(listener)
}

/** Serialize refreshes, coalesce changes during a read, and invalidate stale results. */
export function createAccountRefresh(refresh: (isCurrent: () => boolean) => Promise<void>) {
	let revision = 0
	let disposed = false
	let pending: Promise<void> | undefined

	function request(): Promise<void> {
		if (disposed) return Promise.resolve()
		revision++
		pending ??= Promise.resolve().then(async () => {
			try {
				if (disposed) return
				let completedRevision: number
				do {
					completedRevision = revision
					await refresh(() => !disposed && completedRevision === revision)
				} while (!disposed && completedRevision !== revision)
			} finally {
				pending = undefined
			}
		})
		return pending
	}

	return {
		request,
		dispose: () => {
			disposed = true
			revision++
		},
	}
}
