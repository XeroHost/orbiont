import { AST_NODE_TYPES, parse, type TSESTree } from '@typescript-eslint/typescript-estree'
import { parse as parseVue } from '@vue/compiler-sfc'
import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

/** Count declarations in current source even when English extraction is stale. */
export function sourceMessages(directories: string[]): Record<string, { defaultMessage: string }> {
	const messages: Record<string, { defaultMessage: string }> = {}
	function visit(node: TSESTree.Node) {
		if (node.type === AST_NODE_TYPES.ObjectExpression) {
			const fields = new Map<string, unknown>()
			for (const property of node.properties) {
				if (property.type !== AST_NODE_TYPES.Property || property.computed) continue
				const key =
					property.key.type === AST_NODE_TYPES.Identifier
						? property.key.name
						: property.key.type === AST_NODE_TYPES.Literal
							? property.key.value
							: undefined
				if (typeof key === 'string' && property.value.type === AST_NODE_TYPES.Literal)
					fields.set(key, property.value.value)
			}
			const id = fields.get('id'),
				text = fields.get('defaultMessage')
			if (typeof id === 'string' && typeof text === 'string')
				messages[id] = { defaultMessage: text }
		}
		for (const child of Object.values(node)) {
			if (Array.isArray(child)) {
				for (const value of child)
					if (value && typeof value === 'object' && 'type' in value) visit(value)
			} else if (child && typeof child === 'object' && 'type' in child)
				visit(child as TSESTree.Node)
		}
	}
	function walk(directory: string) {
		for (const entry of readdirSync(directory, { withFileTypes: true })) {
			const file = join(directory, entry.name)
			if (entry.isDirectory()) {
				if (!['node_modules', 'locales'].includes(entry.name) && !entry.name.startsWith('.'))
					walk(file)
			} else if (/\.(vue|[cm]?[jt]sx?)$/.test(file) && !file.endsWith('.d.ts')) {
				const contents = readFileSync(file, 'utf8')
				const scripts = file.endsWith('.vue')
					? (() => {
							const { descriptor } = parseVue(contents, { filename: file })
							return [descriptor.script?.content, descriptor.scriptSetup?.content].filter(
								(value): value is string => !!value,
							)
						})()
					: [contents]
				for (const script of scripts) visit(parse(script, { jsx: /\.[jt]sx$/.test(file) }))
			}
		}
	}
	directories.forEach(walk)
	return messages
}

/** An incomplete catalog must never round up to 100%. */
export function coveragePercentage(complete: number, total: number): number {
	return total === 0 || complete === total
		? 100
		: Math.min(99, Math.round((100 * complete) / total))
}
