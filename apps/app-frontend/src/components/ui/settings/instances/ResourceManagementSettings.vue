<script setup>
import { BoxIcon, FolderOpenIcon, FolderSearchIcon, TrashIcon } from '@orbiont/assets'
import {
	Button,
	defineMessages,
	IconButton,
	injectNotificationManager,
	Input,
	Slider,
	Toggle,
	useSavable,
	useVIntl,
} from '@orbiont/ui'
import { open } from '@tauri-apps/plugin-dialog'
import { ref } from 'vue'

import ConfirmModalWrapper from '@/components/ui/modal/ConfirmModalWrapper.vue'
import ContentStorageSettings from '@/components/ui/settings/instances/ContentStorageSettings.vue'
import DiagnosticsExport from '@/components/ui/settings/instances/DiagnosticsExport.vue'
import { useAppSettings } from '@/composables/use-app-settings.ts'
import { useSettingsChanges } from '@/composables/use-settings-changes'
import { purge_cache_types } from '@/helpers/cache.js'
import { get, set } from '@/helpers/settings.ts'
import { showAppDbBackupsFolder } from '@/helpers/utils.js'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const appSettings = useAppSettings()
const persistedSettings = ref(await get())
const purgeCacheConfirmModal = ref(null)
const alwaysShowCopyDetailsFlag = 'always_show_copy_details'

const draft = useSavable(
	() => ({
		custom_dir: persistedSettings.value.custom_dir,
		max_concurrent_downloads: persistedSettings.value.max_concurrent_downloads,
		max_concurrent_writes: persistedSettings.value.max_concurrent_writes,
		alwaysShowCopyDetails:
			persistedSettings.value.feature_flags[alwaysShowCopyDetailsFlag] ?? false,
	}),
	async (changes) => {
		const { alwaysShowCopyDetails, ...settingsChanges } = changes
		const nextSettings = { ...(await get()), ...settingsChanges }
		if ('custom_dir' in settingsChanges)
			nextSettings.custom_dir = settingsChanges.custom_dir || null
		if (alwaysShowCopyDetails !== undefined) {
			nextSettings.feature_flags = {
				...nextSettings.feature_flags,
				[alwaysShowCopyDetailsFlag]: alwaysShowCopyDetails,
			}
		}
		await set(nextSettings)
		persistedSettings.value = nextSettings
		if (alwaysShowCopyDetails !== undefined) {
			appSettings.featureFlags[alwaysShowCopyDetailsFlag] = alwaysShowCopyDetails
		}
	},
)
const settings = draft.current
useSettingsChanges('resource-management', {
	hasChanges: () => draft.hasChanges.value,
	getOriginal: () => draft.saved.value,
	getModified: () => draft.changes.value,
	isSaving: () => draft.saving.value,
	reset: draft.reset,
	save: draft.save,
})

const messages = defineMessages({
	appDirectoryTitle: {
		id: 'app.settings.resource-management.app-directory.title',
		defaultMessage: 'App directory',
	},
	appDirectoryDescription: {
		id: 'app.settings.resource-management.app-directory.description',
		defaultMessage:
			'Where {productName} stores instances and other files. Changes take effect after restarting the app.',
	},
	selectAppDirectory: {
		id: 'app.settings.resource-management.app-directory.select',
		defaultMessage: 'Select a new app directory',
	},
	browseAppDirectory: {
		id: 'app.settings.resource-management.app-directory.browse',
		defaultMessage: 'Browse for an app directory',
	},
	appCacheTitle: {
		id: 'app.settings.resource-management.app-cache.title',
		defaultMessage: 'App cache',
	},
	purgeCache: {
		id: 'app.settings.resource-management.app-cache.purge',
		defaultMessage: 'Purge cache',
	},
	purgeCacheConfirmTitle: {
		id: 'app.settings.resource-management.app-cache.confirm.title',
		defaultMessage: 'Purge the app cache?',
	},
	purgeCacheConfirmDescription: {
		id: 'app.settings.resource-management.app-cache.confirm.description',
		defaultMessage: 'The app may load more slowly until the cache is rebuilt.',
	},
	appCacheDescription: {
		id: 'app.settings.resource-management.app-cache.description',
		defaultMessage:
			'Clear cached data and download it again. The app may load more slowly until the cache is rebuilt.',
	},
	maximumConcurrentDownloadsTitle: {
		id: 'app.settings.resource-management.maximum-concurrent-downloads.title',
		defaultMessage: 'Maximum concurrent downloads',
	},
	maximumConcurrentDownloadsDescription: {
		id: 'app.settings.resource-management.maximum-concurrent-downloads.description',
		defaultMessage:
			'Number of files the app can download at once. Lower this if downloads are unreliable on your connection. Requires an app restart.',
	},
	maximumConcurrentWritesTitle: {
		id: 'app.settings.resource-management.maximum-concurrent-writes.title',
		defaultMessage: 'Maximum concurrent writes',
	},
	maximumConcurrentWritesDescription: {
		id: 'app.settings.resource-management.maximum-concurrent-writes.description',
		defaultMessage:
			'Number of files the app can write to disk at once. Lower this if you frequently encounter I/O errors. Requires an app restart.',
	},
	alwaysShowCopyDetailsTitle: {
		id: 'app.settings.resource-management.always-show-copy-details.title',
		defaultMessage: 'Always show copy details',
	},
	alwaysShowCopyDetailsDescription: {
		id: 'app.settings.resource-management.always-show-copy-details.description',
		defaultMessage:
			'Show the Copy details action while an install is queued or running. It is always available for failed or interrupted installs.',
	},
	appDatabaseBackupsTitle: {
		id: 'app.settings.resource-management.app-database-backups.title',
		defaultMessage: 'App database backups',
	},
	openBackupsFolder: {
		id: 'app.settings.resource-management.app-database-backups.open-folder',
		defaultMessage: 'Open backups folder',
	},
	appDatabaseBackupsDescription: {
		id: 'app.settings.resource-management.app-database-backups.description',
		defaultMessage:
			'Backups of important app data are stored here in case you need to recover them later.',
	},
})

async function purgeCache() {
	await purge_cache_types([
		'project',
		'project_v3',
		'version',
		'user',
		'team',
		'organization',
		'file',
		'loader_manifest',
		'minecraft_manifest',
		'categories',
		'report_types',
		'loaders',
		'game_versions',
		'donation_platforms',
		'file_hash',
		'file_update',
		'search_results',
		'search_results_v3',
	]).catch(handleError)
}

function handlePurgeCacheClick() {
	if (appSettings.getFeatureFlag('skip_non_essential_warnings')) {
		void purgeCache()
		return
	}

	purgeCacheConfirmModal.value?.show()
}

async function openDbBackupsFolder() {
	await showAppDbBackupsFolder().catch(handleError)
}

async function findLauncherDir() {
	const newDir = await open({
		multiple: false,
		directory: true,
		title: formatMessage(messages.selectAppDirectory),
	})

	if (newDir) {
		settings.value.custom_dir = newDir
	}
}
</script>

<template>
	<div class="flex flex-col gap-6">
		<ContentStorageSettings />
		<DiagnosticsExport />
		<div class="flex flex-col gap-2.5">
			<h2 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.appDirectoryTitle) }}
			</h2>
			<Input
				id="appDir"
				v-model="settings.custom_dir"
				:icon="BoxIcon"
				type="text"
				wrapper-class="w-full"
			>
				<template #right>
					<IconButton
						v-tooltip="formatMessage(messages.browseAppDirectory)"
						:label="formatMessage(messages.browseAppDirectory)"
						class="ml-1.5"
						@click="findLauncherDir"
					>
						<FolderSearchIcon aria-hidden="true" />
					</IconButton>
				</template>
			</Input>
			<p class="m-0 leading-tight text-secondary">
				{{ formatMessage(messages.appDirectoryDescription) }}
			</p>
		</div>

		<div class="flex items-center justify-between gap-4">
			<div>
				<h2 class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.alwaysShowCopyDetailsTitle) }}
				</h2>
				<p class="m-0 mt-1">
					{{ formatMessage(messages.alwaysShowCopyDetailsDescription) }}
				</p>
			</div>
			<Toggle id="always-show-copy-details" v-model="settings.alwaysShowCopyDetails" />
		</div>

		<div class="flex flex-col gap-2.5">
			<ConfirmModalWrapper
				ref="purgeCacheConfirmModal"
				:title="formatMessage(messages.purgeCacheConfirmTitle)"
				:description="formatMessage(messages.purgeCacheConfirmDescription)"
				:has-to-type="false"
				:proceed-label="formatMessage(messages.purgeCache)"
				:show-ad-on-close="false"
				@proceed="purgeCache"
			/>
			<h2 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.appCacheTitle) }}
			</h2>
			<Button id="purge-cache" class="w-fit" @click="handlePurgeCacheClick">
				<TrashIcon aria-hidden="true" />
				{{ formatMessage(messages.purgeCache) }}
			</Button>
			<p class="m-0 leading-tight text-secondary">
				{{ formatMessage(messages.appCacheDescription) }}
			</p>
		</div>

		<div class="flex flex-col gap-2.5">
			<h2 class="m-0 text-lg font-semibold text-contrast mt-4">
				{{ formatMessage(messages.maximumConcurrentDownloadsTitle) }}
			</h2>
			<Slider
				id="max-downloads"
				v-model="settings.max_concurrent_downloads"
				:min="1"
				:max="10"
				:step="1"
			/>
			<p class="m-0 leading-tight text-secondary">
				{{ formatMessage(messages.maximumConcurrentDownloadsDescription) }}
			</p>
		</div>

		<div class="flex flex-col gap-2.5">
			<h2 class="mt-0 m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.maximumConcurrentWritesTitle) }}
			</h2>
			<Slider
				id="max-writes"
				v-model="settings.max_concurrent_writes"
				:min="1"
				:max="50"
				:step="1"
			/>
			<p class="m-0 leading-tight text-secondary">
				{{ formatMessage(messages.maximumConcurrentWritesDescription) }}
			</p>
		</div>

		<div class="flex flex-col gap-2.5">
			<h2 class="mt-0 m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.appDatabaseBackupsTitle) }}
			</h2>
			<Button id="open-db-backups-folder" class="w-fit" @click="openDbBackupsFolder">
				<FolderOpenIcon aria-hidden="true" />
				{{ formatMessage(messages.openBackupsFolder) }}
			</Button>
			<p class="m-0 leading-tight text-secondary">
				{{ formatMessage(messages.appDatabaseBackupsDescription) }}
			</p>
		</div>
	</div>
</template>
