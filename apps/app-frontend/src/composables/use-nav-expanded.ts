import { ref, watch } from 'vue'

// Whether the left navigation shows labels next to its icons. A per-device UI
// preference, so it lives in localStorage; storage can be unavailable (private
// mode, blocked site data), in which case it just defaults to expanded.
const STORAGE_KEY = 'orbiont.nav-expanded'

function readStored(): boolean {
	try {
		return localStorage.getItem(STORAGE_KEY) !== 'false'
	} catch {
		return true
	}
}

const navExpanded = ref(readStored())

watch(navExpanded, (expanded) => {
	try {
		localStorage.setItem(STORAGE_KEY, String(expanded))
	} catch {
		// Not persisted this time; the in-memory state still applies.
	}
})

export function useNavExpanded() {
	return {
		navExpanded,
		toggleNavExpanded: () => {
			navExpanded.value = !navExpanded.value
		},
	}
}
