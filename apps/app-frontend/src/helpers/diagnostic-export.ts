export function createDiagnosticExport({
	save,
	cancel,
	changed,
}: {
	save: (
		operation: string,
		progress: (value: { completed: number; total: number }) => void,
	) => Promise<boolean>
	cancel: (operation: string) => Promise<void>
	changed: (state: { busy: boolean; completed: number; total: number; saved: boolean }) => void
}) {
	let operation: string | null = null
	let state = { busy: false, completed: 0, total: 0, saved: false }
	const notify = () => changed({ ...state })
	return {
		async start() {
			if (operation) return
			const current = crypto.randomUUID()
			operation = current
			state = { busy: true, completed: 0, total: 0, saved: false }
			notify()
			try {
				state.saved = await save(current, (progress) => {
					if (operation !== current) return
					state.completed = Math.max(0, progress.completed)
					state.total = Math.max(state.completed, progress.total)
					notify()
				})
			} finally {
				operation = null
				state.busy = false
				notify()
			}
		},
		async cancel() {
			if (operation) await cancel(operation)
		},
	}
}
