import { prefersReducedMotion } from './motion-preference'

const DURATION_MS = 125
const CLASS = 'theme-color-transitioning'

let removeTimeout: ReturnType<typeof setTimeout> | undefined

export function prepareThemeColorTransition() {
	if (typeof document === 'undefined') return

	const root = document.documentElement
	root.classList.remove(CLASS)
	clearTimeout(removeTimeout)
	removeTimeout = undefined
	if (prefersReducedMotion()) return
	void root.offsetWidth
	root.classList.add(CLASS)

	removeTimeout = setTimeout(() => {
		root.classList.remove(CLASS)
		removeTimeout = undefined
	}, DURATION_MS)
}
