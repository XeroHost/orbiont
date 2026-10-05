import type { Labrinth } from '@orbiont/api-client'
import type { FilterType } from '@orbiont/ui'
import { invoke } from '@tauri-apps/api/core'

import { downloadVersionFile } from './curseforge'

export function bedrockFilterLayout(filters: FilterType[]): FilterType[] {
	return filters
		.map((filter) => ({
			...filter,
			ordering: filter.id.endsWith('_categories')
				? 3
				: filter.id.endsWith('_resolutions')
					? 2
					: filter.id === 'game_version'
						? 1
						: filter.ordering,
			...(filter.id.endsWith('_resolutions')
				? {
						options: [...filter.options].sort((a, b) =>
							a.id.localeCompare(b.id, 'en', { numeric: true }),
						),
					}
				: {}),
			...(filter.id === 'game_version' ? { toggle_groups: [] } : {}),
		}))
		.sort((a, b) => (b.ordering ?? 0) - (a.ordering ?? 0))
}

export interface BedrockInstallRequest {
	project: Labrinth.Projects.v2.Project
	versionId?: string | null
}
let handler: ((request: BedrockInstallRequest) => Promise<string | null>) | undefined
export function setBedrockInstallHandler(value: NonNullable<typeof handler>) {
	handler = value
}
export function requestBedrockInstall(request: BedrockInstallRequest) {
	if (!handler) throw new Error('Bedrock installer unavailable')
	return handler(request)
}
export interface BedrockWorldTarget {
	root_id: string
	path: string
}
export async function importBedrockVersion(
	versionId: string,
	target?: BedrockWorldTarget,
): Promise<number> {
	const match = /^cf-(\d+)-(\d+)$/.exec(versionId)
	if (
		!match ||
		match.slice(1).some((value) => !Number.isSafeInteger(Number(value)) || Number(value) <= 0)
	)
		throw new Error('Invalid Bedrock version ID')
	const path = await downloadVersionFile(versionId)
	return invoke('plugin:bedrock|import_catalog_file', {
		projectId: Number(match[1]),
		fileId: Number(match[2]),
		path,
		...(target ? { target } : {}),
	})
}
