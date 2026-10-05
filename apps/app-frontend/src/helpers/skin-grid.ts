export const SKIN_GRID_GAP = 12
const MIN_CARD_WIDTH = 160
const MAX_COLUMNS = 6

// Measure the gallery itself so every provider adapts to the space beside the preview.
export function getSkinGridColumns(width: number): number {
	if (width <= 0) return MAX_COLUMNS
	return Math.max(
		1,
		Math.min(MAX_COLUMNS, Math.floor((width + SKIN_GRID_GAP) / (MIN_CARD_WIDTH + SKIN_GRID_GAP))),
	)
}
