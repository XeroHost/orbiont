import { computed, readonly, ref } from 'vue'

export type Density = 'comfortable' | 'compact'
export type InterfaceSize = 'small' | 'normal' | 'large'
export type DisplayPreferences = {
	density: Density
	interfaceSize: InterfaceSize
	reduceMotion: boolean
}
export const DEFAULT_DISPLAY_PREFERENCES: DisplayPreferences = {
	density: 'comfortable',
	interfaceSize: 'normal',
	reduceMotion: false,
}

function normalize(value: unknown): DisplayPreferences {
	const input = value && typeof value === 'object' ? (value as Record<string, unknown>) : {}
	return {
		density: input.density === 'compact' ? 'compact' : 'comfortable',
		interfaceSize:
			input.interfaceSize === 'small' || input.interfaceSize === 'large'
				? input.interfaceSize
				: 'normal',
		reduceMotion: input.reduceMotion === true,
	}
}

export function createDisplayPreference(storage: Pick<Storage, 'getItem' | 'setItem'>) {
	let stored: unknown
	try {
		stored = JSON.parse(storage.getItem('orbiont.display') ?? 'null')
	} catch {
		// Invalid or unavailable local preferences use the default appearance.
	}
	const current = ref(normalize(stored))
	const preview = ref<DisplayPreferences | null>(null)
	return {
		current: readonly(current),
		preview,
		effective: computed(() => preview.value ?? current.value),
		set(value: DisplayPreferences) {
			const next = normalize(value)
			storage.setItem('orbiont.display', JSON.stringify(next))
			current.value = next
		},
	}
}

export function getRightPanelLayout(path: string): 'full' | 'catalog' | 'none' {
	if (path.startsWith('/browse') || path.startsWith('/project')) return 'catalog'
	if (['/', '/bedrock', '/skins', '/screenshots'].includes(path) || path.startsWith('/instance/'))
		return 'full'
	return 'none'
}
