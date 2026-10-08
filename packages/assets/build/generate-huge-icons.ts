import fs from 'node:fs'
import path from 'node:path'

import manifest from './huge-icons.json'
import { renderHugeSvg } from './huge-svg'

const packageRoot = path.resolve(__dirname, '..')
const checking = process.argv.includes('--check')

// Render and validate every source before writing any files.
const output = Object.entries(manifest).map(([file, { icon, motion }]) => {
	if (!/^(icons\/|\.\.\/\.\.\/apps\/app-frontend\/src\/assets\/icons\/)[\w/-]+\.svg$/.test(file)) {
		throw new Error(`Invalid icon destination: ${file}`)
	}
	return { file: path.resolve(packageRoot, file), svg: renderHugeSvg(file, icon, motion) }
})
let stale = 0
for (const { file, svg } of output) {
	if (fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n') === svg) continue
	if (checking) {
		console.error(`Outdated Huge icon: ${path.relative(packageRoot, file)}`)
		stale++
	} else fs.writeFileSync(file, svg)
}
if (stale) process.exitCode = 1
else console.log(`${checking ? 'Verified' : 'Generated'} ${output.length} Huge interface icons.`)
