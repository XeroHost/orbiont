import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'

import { renderHugeSvg } from './huge-svg'

const packageRoot = path.resolve(__dirname, '..')
const args = process.argv.slice(2)
const requireIcon = createRequire(import.meta.url)

if (args.includes('--list')) {
	const root = path.resolve(
		path.dirname(requireIcon.resolve('@hugeicons/core-free-icons')),
		'../..',
	)
	const exports = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).exports
	console.log(
		Object.keys(exports)
			.filter((key) => /^\.\/\w+Icon$/.test(key))
			.map((key) => key.slice(2))
			.sort()
			.join('\n'),
	)
} else {
	const [id, icon] = args.filter((arg) => !arg.startsWith('--'))
	if (!id || !icon || !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(id) || !/^\w+Icon$/.test(icon)) {
		console.error(
			'Usage: pnpm icons:add <asset-id> <HugeIcon> [--overwrite]\nExample: pnpm icons:add heart HeartIcon\nUse --list to list available Hugeicons modules.',
		)
		process.exitCode = 1
	} else {
		const file = `icons/${id}.svg`
		const destination = path.join(packageRoot, file)
		if (fs.existsSync(destination) && !args.includes('--overwrite')) {
			throw new Error(`${id} already exists; use --overwrite to replace it.`)
		}
		const svg = renderHugeSvg(file, icon, 'draw')
		const manifestPath = path.join(__dirname, 'huge-icons.json')
		const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'))
		manifest[file] = { icon, motion: 'draw' }
		fs.writeFileSync(destination, svg)
		fs.writeFileSync(
			manifestPath,
			`${JSON.stringify(Object.fromEntries(Object.entries(manifest).sort()), null, '\t')}\n`,
		)
		console.log(`Added ${id} (${icon}). Run pnpm icons:generate to update component exports.`)
	}
}
