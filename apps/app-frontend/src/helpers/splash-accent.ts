import { type Accent, DEFAULT_ACCENT, getAccentPalette } from '@/helpers/accent'

/** Recolor the cyan artwork while preserving neutral pixels and alpha. */
export function getSplashAccentMatrix(accent: Accent): string | null {
	if (accent === DEFAULT_ACCENT) return null
	const channels = getAccentPalette(accent, 'dark')
		.brand.match(/[0-9a-f]{2}/gi)!
		.map((channel) => parseInt(channel, 16))
	const maximum = Math.max(...channels)
	// Cyan energy is (G + B) / 2 - R; neutrals have no cyan energy.
	const matrix = channels.flatMap((channel) => {
		const tint = channel / maximum
		return [1 - tint, tint / 2, tint / 2, 0, 0]
	})
	return [...matrix, 0, 0, 0, 1, 0].join(' ')
}
