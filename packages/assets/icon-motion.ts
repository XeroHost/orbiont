// Parts move independently; every gesture finishes at its resting geometry.
export function partMotion(name: string, index: number): string {
	switch (name) {
		case 'compass':
			return index === 3 ? 'needle' : 'none'
		case 'coffee':
			return index === 2 ? 'steam' : 'none'
		case 'image':
			return index === 0 ? 'spark' : index === 2 ? 'draw' : 'none'
		case 'settings':
			return index === 1 ? 'gear' : 'none'
		case 'sliders':
			return index === 2 || index === 3 || index === 4 ? 'slide' : 'none'
		case 'swords':
			return index === 0 ? 'sword-left' : 'sword-right'
		case 'chart':
			return index < 3 ? 'bar' : 'none'
		case 'magic':
			return index === 0 ? 'wand' : 'spark'
		case 'lightbulb':
			return index < 2 ? 'spark' : 'none'
		case 'cpu':
			return index === 0 ? 'spark' : 'draw'
		case 'play':
			return 'advance'
		case 'shirt':
			return index === 1 ? 'sway' : 'draw'
		case 'refresh':
			return 'gear'
		case 'paintbrush':
		case 'feather':
			return 'sway'
		case 'zap':
			return 'spark'
		case 'users':
			return index % 2 === 0 ? 'lift' : 'draw'
		default:
			return 'draw'
	}
}
