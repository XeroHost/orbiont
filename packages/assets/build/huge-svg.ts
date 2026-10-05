import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'

import { partMotion } from '../icon-motion'

type IconData = [string, Record<string, string>][]
const requireIcon = createRequire(import.meta.url)
const packageRoot = path.resolve(__dirname, '..')
const escape = (value: string) =>
	value.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;')

export function renderHugeSvg(file: string, icon: string, motion: string): string {
	const data = requireIcon(`@hugeicons/core-free-icons/${icon}`) as IconData
	if (!data) throw new Error(`Missing Hugeicons module: ${icon}`)
	const destination = path.resolve(packageRoot, file)
	const current = fs.existsSync(destination) ? fs.readFileSync(destination, 'utf8') : ''
	const root = current.match(/<svg\b[^>]*>/)?.[0] ?? ''
	const size = (attribute: string) =>
		root.match(new RegExp(`(?:^|\\s)${attribute}="([^"]+)"`))?.[1] ?? '24'
	const elements = data.map(([tag, original], index) => {
		const attributes = { ...original }
		delete attributes.key
		// Status indicators retain the caller's semantic color variables.
		if (file === 'icons/online-indicator.svg') {
			attributes.fill =
				index === 0
					? 'var(--_color-outer, var(--color-green-highlight))'
					: 'var(--_color-inner, var(--color-green))'
			attributes.stroke = 'none'
		}
		if (file === 'icons/radio-button-checked.svg' && index === 1) attributes.fill = 'currentColor'
		const gesture = partMotion(motion, index)
		attributes.class = 'huge-icon-part'
		attributes.pathLength = '1'
		attributes.style = `--icon-motion:${gesture === 'none' ? 'none' : `huge-${gesture}`};--icon-delay:${(index % 3) * 40}ms`
		const serialized = Object.entries(attributes)
			.map(([key, value]) => {
				const name =
					key === 'pathLength' ? key : key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)
				return `${name}="${escape(value)}"`
			})
			.join(' ')
		return `  <${tag} ${serialized}/>`
	})
	// A few legacy actions combine a server/group with an add/search badge.
	// Both shapes come from Hugeicons, preserving the action's meaning.
	const badges: Record<string, string> = {
		'icons/server-plus.svg': 'AddCircleIcon',
		'icons/server-search.svg': 'Search01Icon',
		'icons/organization-plus.svg': 'AddCircleIcon',
	}
	if (badges[file]) {
		const badge = requireIcon(`@hugeicons/core-free-icons/${badges[file]}`) as IconData
		const shapes = badge
			.map(
				([tag, attributes]) =>
					`<${tag} ${Object.entries(attributes)
						.filter(([key]) => key !== 'key')
						.map(
							([key, value]) =>
								`${key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}="${escape(value)}"`,
						)
						.join(' ')}/>`,
			)
			.join('')
		elements.push(
			`  <g transform="translate(14 14) scale(.4)"><circle cx="12" cy="12" r="12" fill="var(--surface-3, #27292e)"/>${shapes}</g>`,
		)
	}
	return `<svg xmlns="http://www.w3.org/2000/svg" width="${size('width')}" height="${size('height')}" viewBox="0 0 24 24" fill="none" class="huge-animated-icon" data-huge-icon="${motion}" data-huge-source="${icon}" aria-hidden="true" focusable="false">\n${elements.join('\n')}\n</svg>\n`
}
