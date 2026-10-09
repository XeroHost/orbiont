import { computed, watch } from 'vue'

import { useTheme } from '@/composables/use-theme'
import { createAccentPreference, getAccentPalette } from '@/helpers/accent'

const preference = createAccentPreference({
	getItem: (key) => window.localStorage.getItem(key),
	setItem: (key, value) => window.localStorage.setItem(key, value),
})
const theme = useTheme()
const palette = computed(() => getAccentPalette(preference.effective.value, theme.active))

watch(
	palette,
	(value) => {
		const style = document.documentElement.style
		style.setProperty('--color-brand', value.brand)
		style.setProperty('--color-brand-highlight', value.highlight)
		style.setProperty('--color-brand-shadow', value.shadow)
		style.setProperty('--color-focus-ring', value.focus)
		style.setProperty(
			'--loading-bar-gradient',
			`linear-gradient(to right, ${value.brand}, ${value.focus})`,
		)
	},
	{ immediate: true, flush: 'sync' },
)

export function useAccent() {
	return { ...preference, palette }
}
