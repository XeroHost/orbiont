import { computed, readonly, ref, watch } from 'vue'

export type IconMotion = 'on' | 'off'
const STORAGE_KEY = 'orbiont.icon-motion'

function readStored(): IconMotion {
	try {
		const value = localStorage.getItem(STORAGE_KEY)
		// Missing preferences and the former system mode now default to enabled.
		return value === 'off' ? 'off' : 'on'
	} catch {
		return 'on'
	}
}

const current = ref<IconMotion>(readStored())
const preview = ref<IconMotion | null>(null)
const effective = computed(() => preview.value ?? current.value)

watch(
	effective,
	(value) => {
		if (typeof document !== 'undefined') document.documentElement.dataset.iconMotion = value
	},
	{ immediate: true },
)

export function useIconMotion() {
	return {
		current: readonly(current),
		preview,
		set(value: IconMotion) {
			current.value = value
			try {
				localStorage.setItem(STORAGE_KEY, value)
			} catch {
				// The in-memory choice still applies if storage is unavailable.
			}
		},
	}
}
