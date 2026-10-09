import { computed, watch } from 'vue'

import { createDisplayPreference } from '@/helpers/display-preferences'

const preference = createDisplayPreference({
	getItem: (key) => localStorage.getItem(key),
	setItem: (key, value) => localStorage.setItem(key, value),
})
// The explicit launcher setting takes precedence over the operating system.
const reducedMotion = computed(() => preference.effective.value.reduceMotion)

watch(
	[preference.effective, reducedMotion],
	([value, reduced]) => {
		if (typeof document === 'undefined') return
		const root = document.documentElement
		root.dataset.density = value.density
		root.dataset.interfaceSize = value.interfaceSize
		root.dataset.reducedMotion = reduced ? 'on' : 'off'
	},
	{ immediate: true, flush: 'sync' },
)

export function isMotionReduced(): boolean {
	return reducedMotion.value
}

export function useDisplayPreferences() {
	return { ...preference, reducedMotion }
}
