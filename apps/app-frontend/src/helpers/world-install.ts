import type { Labrinth } from '@orbiont/api-client'
import { invoke } from '@tauri-apps/api/core'

export type WorldInstallConflict = { path: string; name: string; fingerprint: string }
export type WorldInstallPreview = {
	folder: string
	name: string
	conflicts: WorldInstallConflict[]
}
export type WorldInstallResult = {
	path: string | null
	backup: string | null
	preview: WorldInstallPreview | null
}
export type WorldInstallAction = 'create' | 'replace' | 'copy' | 'keep' | 'cancel'
export type WorldInstallRequest = {
	project: Labrinth.Projects.v2.Project
	versions: Labrinth.Versions.v2.Version[]
	instanceId?: string | null
	autoInstall?: boolean
	allowIncompatible?: boolean
}
let handler: ((request: WorldInstallRequest) => Promise<string | null>) | undefined
export function setWorldInstallHandler(value: NonNullable<typeof handler>) {
	handler = value
}
export function requestWorldInstall(request: WorldInstallRequest) {
	if (!handler) throw new Error('World installer unavailable')
	return handler(request)
}
export function inspectWorldArchive(instanceId: string, archivePath: string, projectId: string) {
	return invoke<WorldInstallPreview>('plugin:worlds|inspect_world_archive', {
		instanceId,
		archivePath,
		projectId,
	})
}
export function importWorldArchive(
	instanceId: string,
	archivePath: string,
	projectId: string,
	action: WorldInstallAction,
	conflict?: WorldInstallConflict,
) {
	return invoke<WorldInstallResult>('plugin:worlds|import_world_archive', {
		instanceId,
		archivePath,
		projectId,
		action,
		conflict,
	})
}
