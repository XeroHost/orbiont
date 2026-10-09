/** The host may enable reduced motion; the operating system preference always takes precedence. */
export function prefersReducedMotion(): boolean {
	const hostPreference =
		typeof document !== 'undefined' ? document.documentElement.dataset.reducedMotion : undefined
	if (hostPreference === 'on' || hostPreference === 'off') return hostPreference === 'on'
	return (
		typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches
	)
}
