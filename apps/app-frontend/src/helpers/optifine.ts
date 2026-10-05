import { defineMessages } from '@orbiont/ui'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { openUrl } from '@tauri-apps/plugin-opener'

import type { InstallJobSnapshot } from '@/generated/app-events/InstallJobSnapshot'
import i18n from '@/i18n.config'

import { matchesOptifineSelection, type OptifineReference } from './optifine-selection'

export const optifineMessages = defineMessages({
	version: {
		id: 'optifine.component-version',
		defaultMessage: '{version} · Minecraft {gameVersion}',
	},
	select: { id: 'optifine.select-installer', defaultMessage: 'Select official OptiFine JAR' },
	mismatch: {
		id: 'optifine.incompatible-installer',
		defaultMessage:
			'Select the official OptiFine installer for Minecraft {gameVersion}. For an imported pack, its version and file hash must also match.',
	},
	canceled: {
		id: 'optifine.import-canceled',
		defaultMessage: 'OptiFine selection canceled. The pack was not installed.',
	},
	importTitle: { id: 'optifine.import-title', defaultMessage: 'This pack requires OptiFine' },
	importDescription: {
		id: 'optifine.import-description',
		defaultMessage:
			'This pack requires OptiFine {version} for Minecraft {gameVersion}. Download its official JAR and select it to continue. OptiFine is not bundled in the pack.',
	},
	download: { id: 'optifine.download-official', defaultMessage: 'Official download' },
	continue: { id: 'optifine.continue-import', defaultMessage: 'Select JAR and continue' },
	exportNotice: {
		id: 'optifine.export-notice',
		defaultMessage:
			'OptiFine binaries are excluded. Orbpack preserves its required version; Modrinth and CurseForge packs require installing OptiFine separately.',
	},
	remove: { id: 'optifine.remove-component', defaultMessage: 'Remove OptiFine' },
	add: { id: 'optifine.add-component', defaultMessage: 'Install OptiFine' },
})

export async function openOptifineDownloads() {
	const url = await invoke<string>('plugin:install|install_optifine_download_url')
	await openUrl(url)
}

export async function getInstanceOptifine(instanceId: string) {
	return invoke<OptifineReference | null>('plugin:install|install_get_instance_optifine', {
		instanceId,
	})
}

export async function changeInstanceOptifine(instanceId: string, installerPath: string | null) {
	return invoke<InstallJobSnapshot>('plugin:install|install_change_optifine', {
		instanceId,
		installerPath,
	})
}

export async function pickOptifineInstaller(gameVersion: string, expected?: OptifineReference) {
	const path = await open({
		multiple: false,
		title: i18n.global.t(optifineMessages.select.id),
		filters: [{ name: 'OptiFine', extensions: ['jar'] }],
	})
	if (typeof path !== 'string') return null
	const reference = await invoke<OptifineReference>('plugin:install|install_inspect_optifine', {
		path,
	})
	if (!matchesOptifineSelection(reference, gameVersion, 'vanilla', expected)) {
		throw new Error(i18n.global.t(optifineMessages.mismatch.id, { gameVersion }))
	}
	return { path, reference }
}

let importHandler: ((reference: OptifineReference) => Promise<string | null>) | undefined
export function setOptifineImportHandler(handler: NonNullable<typeof importHandler>) {
	importHandler = handler
}

export async function requestOptifineForPack(location: { type: string; path?: string }) {
	if (location.type !== 'fromFile' || !location.path) return null
	const reference = await invoke<OptifineReference | null>(
		'plugin:install|install_get_pack_optifine',
		{ path: location.path },
	)
	if (!reference) return null
	const path = await importHandler?.(reference)
	if (!path) throw new Error(i18n.global.t(optifineMessages.canceled.id))
	return path
}
