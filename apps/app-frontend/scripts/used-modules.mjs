// Builds the frontend and lists every source file that ends up in the bundle
// with rendered code (tree-shaken modules don't count). Used to find dead
// code across the workspace packages. Usage: node scripts/used-modules.mjs out.json
import { writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { build } from 'vite'

const outFile = resolve(process.argv[2] ?? 'used-modules.json')
const used = new Set()

await build({
	configFile: resolve(import.meta.dirname, '../vite.config.ts'),
	logLevel: 'warn',
	build: { outDir: join(tmpdir(), 'orbiont-used-modules'), emptyOutDir: true, minify: false },
	plugins: [
		{
			name: 'collect-used-modules',
			generateBundle(_options, bundle) {
				for (const chunk of Object.values(bundle)) {
					if (chunk.type !== 'chunk') continue
					for (const [id, info] of Object.entries(chunk.modules)) {
						if (info.renderedLength > 0) used.add(id.split('?')[0].replaceAll('\\', '/'))
					}
				}
			},
		},
	],
})

writeFileSync(outFile, JSON.stringify([...used].sort(), null, 2))
console.log(`${used.size} modules with rendered code -> ${outFile}`)
