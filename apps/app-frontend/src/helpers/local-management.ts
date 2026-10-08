import { invoke } from '@tauri-apps/api/core'

import {
	getBedrockStorage,
	listBedrockRecoveryPage,
	previewBedrockRecovery,
	removeBedrockStorage,
	restoreBedrockItem,
} from './bedrock'

export interface LocalCopy {
	id: string
	rootId: string
	path: string
	created: number
	size: number
	limited: boolean
	state: string
	source?: string
	removable: boolean
}
export interface LocalPreview {
	copy: LocalCopy
	canRestore: boolean
	conflicts: string[]
	content?: string
	expectedRevision?: string
}
export interface LocalStorageEntry extends LocalCopy {
	category: string
}
export interface LocalManagementAdapter {
	edition: 'java' | 'bedrock'
	list: (offset: number) => Promise<{ entries: LocalCopy[]; total: number; incomplete: boolean }>
	preview: (copy: LocalCopy) => Promise<LocalPreview>
	restore: (preview: LocalPreview) => Promise<void>
	storage: () => Promise<{
		categories: Record<string, number>
		entries: LocalStorageEntry[]
		incomplete: boolean
	}>
	remove: (entries: LocalStorageEntry[]) => Promise<void>
}

export function createJavaManagementAdapter(
	instanceId: () => string,
	changed: () => Promise<unknown>,
): LocalManagementAdapter {
	const copy = (entry: { id: string; path: string; created: number; size: number }): LocalCopy => ({
		...entry,
		rootId: instanceId(),
		state: 'available',
		source: 'java_editor',
		limited: false,
		removable: true,
	})
	const list = async () => {
		const result = await invoke<{
			entries: { id: string; path: string; created: number; size: number }[]
			incomplete: boolean
		}>('plugin:files|file_list_recoveries', { instanceId: instanceId() })
		return {
			entries: result.entries.map(copy),
			total: result.entries.length,
			incomplete: result.incomplete,
		}
	}
	return {
		edition: 'java',
		list,
		preview: async (selected) => {
			const result = await invoke<{ content: string; currentRevision: string; path: string }>(
				'plugin:files|file_preview_recovery',
				{ instanceId: selected.rootId, recoveryId: selected.id },
			)
			return {
				copy: { ...selected, path: result.path },
				canRestore: true,
				conflicts: [],
				content: result.content.slice(0, 16384),
				expectedRevision: result.currentRevision,
			}
		},
		restore: async (preview) => {
			await invoke('plugin:files|file_restore_recovery', {
				instanceId: preview.copy.rootId,
				recoveryId: preview.copy.id,
				expectedRevision: preview.expectedRevision,
			})
			await changed()
		},
		storage: async () => {
			const [summary, copies] = await Promise.all([
				invoke<{ dataBytes: number; cacheBytes: number; backupBytes: number; incomplete: boolean }>(
					'plugin:files|file_storage_summary',
					{ instanceId: instanceId() },
				),
				list(),
			])
			return {
				categories: {
					data: summary.dataBytes,
					cache: summary.cacheBytes,
					recovery: summary.backupBytes,
				},
				entries: copies.entries.map((entry) => ({ ...entry, category: 'recovery' })),
				incomplete: summary.incomplete || copies.incomplete,
			}
		},
		remove: async (entries) => {
			if (
				!entries.length ||
				entries.some(
					(entry) =>
						!entry.removable || entry.category !== 'recovery' || entry.rootId !== instanceId(),
				)
			)
				throw new Error('Invalid storage selection')
			await invoke('plugin:files|file_remove_recoveries', {
				instanceId: instanceId(),
				recoveryIds: entries.map((entry) => entry.id),
			})
			await changed()
		},
	}
}

export function createBedrockManagementAdapter(
	changed: () => Promise<unknown>,
): LocalManagementAdapter {
	const copy = (
		entry: Awaited<ReturnType<typeof listBedrockRecoveryPage>>['items'][number],
	): LocalCopy => ({
		id: entry.id,
		rootId: entry.root_id,
		path: entry.path,
		created: entry.saved_at,
		size: entry.size_bytes,
		limited: entry.size_limited,
		state: entry.state,
		source: entry.source,
		removable: false,
	})
	return {
		edition: 'bedrock',
		list: async (offset) => {
			const result = await listBedrockRecoveryPage(offset)
			return { entries: result.items.map(copy), total: result.total, incomplete: result.limited }
		},
		preview: async (selected) => {
			const result = await previewBedrockRecovery(selected.rootId, selected.id)
			return {
				copy: copy(result.recovery),
				canRestore: result.can_restore,
				conflicts: result.conflicts,
			}
		},
		restore: async (preview) => {
			await restoreBedrockItem(preview.copy.rootId, preview.copy.id)
			await changed()
		},
		storage: async () => {
			const result = await getBedrockStorage()
			const categories: Record<string, number> = {}
			const entries = result.entries.map((entry) => {
				categories[entry.category] = (categories[entry.category] ?? 0) + entry.size_bytes
				return {
					id: entry.id,
					rootId: entry.root_id,
					path: entry.path,
					category: entry.category,
					size: entry.size_bytes,
					limited: entry.size_limited,
					created: 0,
					state: 'available',
					removable: entry.removable && ['recovery', 'import'].includes(entry.category),
				}
			})
			return {
				entries,
				categories,
				incomplete: result.limited || entries.some((entry) => entry.limited),
			}
		},
		remove: async (entries) => {
			if (
				!entries.length ||
				entries.some(
					(entry) => !entry.removable || !['recovery', 'import'].includes(entry.category),
				)
			)
				throw new Error('Invalid storage selection')
			try {
				for (const entry of entries) await removeBedrockStorage(entry.rootId, entry.id)
			} finally {
				await changed()
			}
		},
	}
}
