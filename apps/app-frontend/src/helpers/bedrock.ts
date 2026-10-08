import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

import { type BedrockStopOutcome, requestBedrockStop } from './bedrock-stop'

export interface BedrockApplication {
	version: string
	can_launch: boolean
}
export interface BedrockLaunch {
	state: 'idle' | 'requested' | 'starting' | 'running' | 'timeout' | 'failed'
	elapsed_ms: number
	diagnostic: string | null
}
export function isBedrockLaunchPending(launch: BedrockLaunch | null | undefined): boolean {
	return launch?.state === 'requested' || launch?.state === 'starting'
}

export interface BedrockStatus {
	supported: boolean
	game_running: boolean
	launch: BedrockLaunch
	game: BedrockApplication | null
	launcher: BedrockApplication | null
}

export type BedrockItemKind = 'resource_pack' | 'skin_pack' | 'behavior_pack' | 'world' | 'log'
export interface BedrockRoot {
	id: string
	path: string
	kind: 'shared' | 'user' | 'legacy' | 'logs'
}
export interface BedrockItem {
	root_id: string
	path: string
	name: string
	kind: BedrockItemKind
	version: string | null
	description: string | null
	pack_id: string | null
	pack_version: number[] | null
	dependencies: { pack_id: string; version: number[] }[]
	activations: { root_id: string; world_path: string; version: number[] }[]
	active: boolean
	development: boolean
	icon_path?: string | null
}
export interface BedrockWorkspace {
	roots: BedrockRoot[]
	items: BedrockItem[]
	incomplete: boolean
}
export interface BedrockFile {
	name: string
	path: string
	is_dir: boolean
	size: number
	created: number | null
	modified: number | null
	count: number | null
}
export interface BedrockDirectory {
	entries: BedrockFile[]
	limited: boolean
}
export interface BedrockLog {
	text: string
	truncated: boolean
}

export function getBedrockStatus(refresh = false): Promise<BedrockStatus> {
	return invoke('plugin:bedrock|get_status', { refresh })
}
export function launchBedrock(): Promise<void> {
	return invoke('plugin:bedrock|launch_game')
}
export function stopBedrock(confirmForce?: () => Promise<boolean>): Promise<BedrockStopOutcome> {
	return requestBedrockStop(
		(forceToken) => invoke('plugin:bedrock|stop_game', { forceToken }),
		confirmForce ?? (() => Promise.resolve(false)),
	)
}

export function bedrockStatusQueryOptions() {
	return {
		queryKey: ['bedrock', 'status'],
		queryFn: () => getBedrockStatus(),
		staleTime: 5000,
		refetchOnWindowFocus: 'always' as const,
		retry: false as const,
	}
}
export function launchOfficialMinecraftLauncher(): Promise<void> {
	return invoke('plugin:bedrock|launch_official_launcher')
}
export function openBedrockStore(): Promise<void> {
	return invoke('plugin:bedrock|open_store')
}
export function openBedrockUpdates(): Promise<void> {
	return invoke('plugin:bedrock|open_updates')
}
export function getBedrockWorkspace(refresh = false): Promise<BedrockWorkspace> {
	return invoke('plugin:bedrock|get_workspace', { refresh })
}
export function listBedrockFiles(rootId: string, path = ''): Promise<BedrockDirectory> {
	return invoke('plugin:bedrock|list_files', { rootId, path })
}
export function openBedrockFolder(rootId: string, path = ''): Promise<void> {
	return invoke('plugin:bedrock|open_folder', { rootId, path })
}
export function readBedrockLog(rootId: string, path: string): Promise<BedrockLog> {
	return invoke('plugin:bedrock|read_log', { rootId, path })
}

export interface BedrockDocument {
	text: string
	revision: string
}
export interface BedrockRecovery {
	id: string
	root_id: string
	path: string
	saved_at: number
	operation: string
	state: string
	source: string
	size_bytes: number
	size_limited: boolean
	diagnostic: string | null
}
export function readBedrockFile(rootId: string, path: string): Promise<BedrockDocument> {
	return invoke('plugin:bedrock|read_file', { rootId, path })
}
export function writeBedrockFile(
	rootId: string,
	path: string,
	text: string,
	revision: string,
): Promise<BedrockDocument> {
	return invoke('plugin:bedrock|write_file', { rootId, path, text, revision })
}
export function deleteBedrockItem(rootId: string, path: string): Promise<BedrockRecovery> {
	return invoke('plugin:bedrock|delete_item', { rootId, path })
}
export function listBedrockRecoveries(): Promise<BedrockRecovery[]> {
	return invoke('plugin:bedrock|list_recoveries')
}
export function restoreBedrockItem(rootId: string, id: string): Promise<void> {
	return invoke('plugin:bedrock|restore_item', { rootId, id })
}

/** True means handed to Minecraft. The game reports the actual import result. */
export async function pickAndImportBedrockFile(filterName: string): Promise<boolean> {
	const path = await open({
		multiple: false,
		directory: false,
		filters: [{ name: filterName, extensions: ['mcworld', 'mcpack', 'mcaddon'] }],
	})
	if (!path || typeof path !== 'string') return false
	await invoke('plugin:bedrock|import_file', { path })
	return true
}

export function getBedrockProcessStatus(): Promise<Pick<BedrockStatus, 'game_running' | 'launch'>> {
	return invoke('plugin:bedrock|get_process_status')
}
export function listBedrockRecoveryPage(
	offset = 0,
	limit = 100,
): Promise<{ items: BedrockRecovery[]; total: number; limited: boolean }> {
	return invoke('plugin:bedrock|list_recoveries_page', { offset, limit })
}
export function previewBedrockRecovery(
	rootId: string,
	id: string,
): Promise<{ recovery: BedrockRecovery; can_restore: boolean; conflicts: string[] }> {
	return invoke('plugin:bedrock|preview_recovery', { rootId, id })
}
export interface BedrockStorageEntry {
	id: string
	root_id: string
	path: string
	category: string
	size_bytes: number
	size_limited: boolean
	removable: boolean
}
export function getBedrockStorage(): Promise<{ entries: BedrockStorageEntry[]; limited: boolean }> {
	return invoke('plugin:bedrock|get_storage')
}
export function removeBedrockStorage(rootId: string, id: string): Promise<void> {
	return invoke('plugin:bedrock|remove_storage', { rootId, id })
}
