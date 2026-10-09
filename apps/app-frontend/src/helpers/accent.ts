import { computed, readonly, ref } from 'vue'

export const ACCENT_OPTIONS = [
	'cyan',
	'sky',
	'blue',
	'indigo',
	'violet',
	'pink',
	'red',
	'coral',
	'orange',
	'amber',
	'yellow',
	'lime',
	'green',
	'emerald',
	'teal',
] as const
export type Accent = (typeof ACCENT_OPTIONS)[number]
export const DEFAULT_ACCENT: Accent = 'cyan'
const STORAGE_KEY = 'orbiont.accent'

const palettes: Record<Accent, { dark: string; light: string }> = {
	cyan: { dark: '#00c3c3', light: '#007d7d' },
	sky: { dark: '#38bdf8', light: '#0369a1' },
	blue: { dark: '#528bff', light: '#2563eb' },
	indigo: { dark: '#818cf8', light: '#4338ca' },
	violet: { dark: '#a78bfa', light: '#7c3aed' },
	pink: { dark: '#f472b6', light: '#be185d' },
	red: { dark: '#f87171', light: '#b91c1c' },
	coral: { dark: '#fb7185', light: '#be123c' },
	orange: { dark: '#fb923c', light: '#c2410c' },
	amber: { dark: '#fbbf24', light: '#92400e' },
	yellow: { dark: '#fde047', light: '#a16207' },
	lime: { dark: '#a3e635', light: '#4d7c0f' },
	green: { dark: '#4ade80', light: '#15803d' },
	emerald: { dark: '#34d399', light: '#047857' },
	teal: { dark: '#2dd4bf', light: '#0f766e' },
}

export function isAccent(value: unknown): value is Accent {
	return typeof value === 'string' && (ACCENT_OPTIONS as readonly string[]).includes(value)
}

export function getAccentPalette(accent: Accent, theme: string) {
	const palette = palettes[accent]
	const brand = theme === 'light' || theme === 'retro' ? palette.light : palette.dark
	const rgb = brand
		.match(/[0-9a-f]{2}/gi)!
		.map((channel) => parseInt(channel, 16))
		.join(', ')
	return {
		brand,
		focus: theme === 'retro' ? palette.dark : brand,
		highlight: `rgba(${rgb}, 0.25)`,
		shadow: `rgba(${rgb}, 0.7)`,
		soft: `rgba(${rgb}, 0.12)`,
	}
}

export function createAccentPreference(storage: Pick<Storage, 'getItem' | 'setItem'>) {
	let stored: unknown
	try {
		stored = storage.getItem(STORAGE_KEY)
	} catch {
		// Start with the default if local preferences cannot be read.
	}
	const current = ref<Accent>(isAccent(stored) ? stored : DEFAULT_ACCENT)
	const preview = ref<Accent | null>(null)
	return {
		current: readonly(current),
		preview,
		effective: computed(() => preview.value ?? current.value),
		set(value: Accent) {
			// A failed write keeps the draft unsaved, so the user can retry.
			storage.setItem(STORAGE_KEY, value)
			current.value = value
		},
	}
}
