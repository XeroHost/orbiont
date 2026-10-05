import { productName } from '@orbiont/branding'

export type PackExportFormat = 'orbpack' | 'mrpack' | 'curseforge'

export const PACK_FORMATS = [
	{ value: 'orbpack', extension: 'orbpack', provider: productName },
	{ value: 'mrpack', extension: 'mrpack', provider: 'Modrinth' },
	{ value: 'curseforge', extension: 'zip', provider: 'CurseForge' },
] as const

export function getPackImportFilters(allFormatsLabel: string) {
	return [
		{ name: allFormatsLabel, extensions: PACK_FORMATS.map((format) => format.extension) },
		...PACK_FORMATS.map((format) => ({ name: format.provider, extensions: [format.extension] })),
	]
}

export function getPackSaveFilters(selected: PackExportFormat) {
	return [...PACK_FORMATS]
		.sort((a, b) => Number(b.value === selected) - Number(a.value === selected))
		.map((format) => ({ name: format.provider, extensions: [format.extension] }))
}

export function resolvePackExport(path: string, selected: PackExportFormat) {
	const explicit = PACK_FORMATS.find((format) =>
		path.toLowerCase().endsWith(`.${format.extension}`),
	)
	if (explicit) return { path, format: explicit.value }
	const format = PACK_FORMATS.find((format) => format.value === selected)!
	return { path: `${path}.${format.extension}`, format: format.value }
}
