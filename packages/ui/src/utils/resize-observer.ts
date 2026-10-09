// Layout writes during ResizeObserver delivery can resize an ancestor that was
// already visited. Apply measurements in the next frame to avoid that feedback loop.
export function createFrameResizeObserver(callback: ResizeObserverCallback) {
	const pending = new Map<Element, ResizeObserverEntry>()
	let frame: number | undefined
	const observer = new ResizeObserver((entries) => {
		for (const entry of entries) pending.set(entry.target, entry)
		frame ??= requestAnimationFrame(() => {
			frame = undefined
			const latest = [...pending.values()]
			pending.clear()
			if (latest.length) callback(latest, observer)
		})
	})
	return {
		observe: (target: Element, options?: ResizeObserverOptions) =>
			observer.observe(target, options),
		unobserve(target: Element) {
			pending.delete(target)
			observer.unobserve(target)
		},
		disconnect() {
			observer.disconnect()
			if (frame !== undefined) cancelAnimationFrame(frame)
			frame = undefined
			pending.clear()
		},
	}
}
