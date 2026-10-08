import { gzipSync } from 'node:zlib'
import { readFile, readdir } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

const root = fileURLToPath(new URL('../', import.meta.url))
const directory = path.join(root, 'apps/app-frontend/dist')
// Approximately 20% headroom above the measured release build (6.62 / 1.87 MiB).
const limits = { javascriptBytes: 8 * 1024 * 1024, aceSourceBytes: 2.25 * 1024 * 1024 }

export async function measureBundle(folder) {
	const result = { javascriptFiles: 0, javascriptBytes: 0, gzipBytes: 0, aceAuxiliaryBytes: 0 }
	async function visit(current) {
		for (const entry of await readdir(current, { withFileTypes: true })) {
			// Build outputs should never require following filesystem links.
			if (entry.isSymbolicLink()) continue
			const file = path.join(current, entry.name)
			if (entry.isDirectory()) await visit(file)
			else if (entry.isFile() && entry.name.endsWith('.js')) {
				const bytes = await readFile(file)
				result.javascriptFiles++
				result.javascriptBytes += bytes.length
				result.gzipBytes += gzipSync(bytes).length
				if (/^(mode-|worker-|theme-|ext-)/.test(entry.name))
					result.aceAuxiliaryBytes += bytes.length
			}
		}
	}
	await visit(folder)
	const ace = JSON.parse(await readFile(path.join(folder, 'bundle-metrics.json'), 'utf8'))
	if (
		!Number.isSafeInteger(ace.aceSourceBytes) ||
		ace.aceSourceBytes < 0 ||
		!Number.isSafeInteger(ace.aceModules) ||
		ace.aceModules <= 0
	) {
		throw new Error('Missing or invalid Ace module measurements from the frontend build')
	}
	return { ...result, aceSourceBytes: ace.aceSourceBytes, aceModules: ace.aceModules }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	const metrics = await measureBundle(directory)
	console.log(JSON.stringify({ metrics, limits }, null, 2))
	if (process.argv.includes('--check')) {
		const exceeded = Object.entries(limits).filter(([key, value]) => metrics[key] > value)
		if (exceeded.length) {
			console.error(`Frontend budget exceeded: ${exceeded.map(([key]) => key).join(', ')}`)
			process.exitCode = 1
		}
	}
}
