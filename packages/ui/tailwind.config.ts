import preset from '@orbiont/tooling-config/tailwind/tailwind-preset.ts'
import type { Config } from 'tailwindcss'

const config: Config = {
	content: ['./src/**/*.{js,vue,ts}'],
	presets: [preset],
}

export default config
