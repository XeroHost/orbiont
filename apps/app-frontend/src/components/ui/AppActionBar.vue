<template>
	<div class="flex gap-2 items-center">
		<DownloadManager />
		<div v-if="downloadState.total > 0 || hasActiveLoadingBars" class="relative">
			<IconButton
				v-tooltip="downloadToggleLabel"
				:color="downloadState.hidden > 0 ? 'brand' : undefined"
				type="quiet"
				:label="downloadToggleLabel"
				@click="toggleDownloadNotifications"
			>
				<DownloadIcon />
			</IconButton>
		</div>
		<div v-if="offline" class="flex items-center gap-1">
			<UnplugIcon class="text-secondary" />
			<span class="text-sm text-contrast"> {{ formatMessage(messages.offline) }} </span>
		</div>
		<AppUpdateButton />
		<div
			class="flex border-solid border-surface-5 text-sm font-medium items-center gap-2 py-1.5 px-3 rounded-xl border"
		>
			<template v-if="selectedProcess">
				<OnlineIndicatorIcon />
				<div class="text-contrast flex items-center gap-2">
					<router-link
						v-tooltip="formatMessage(messages.viewInstance)"
						:to="processRoute(selectedProcess)"
						class="hover:underline"
					>
						{{ selectedProcess.instance.name }}
					</router-link>
					<FloatingMenu
						v-if="currentProcesses.length > 1"
						placement="bottom"
						@open="showInstances = true"
						@close="showInstances = false"
					>
						<IconButton
							v-tooltip="
								showInstances
									? formatMessage(messages.hideMoreRunningInstances)
									: formatMessage(messages.showMoreRunningInstances)
							"
							class="!size-6"
							type="quiet"
							size="xs"
							:label="
								showInstances
									? formatMessage(messages.hideMoreRunningInstances)
									: formatMessage(messages.showMoreRunningInstances)
							"
						>
							<DropdownIcon :class="{ 'rotate-180': !!showInstances }" />
						</IconButton>
						<template #popper>
							<div class="flex w-[20rem] max-h-[24rem] flex-col gap-2 overflow-auto">
								<div
									v-for="process in currentProcesses"
									:key="process.uuid"
									class="flex w-full items-center gap-2 rounded-xl bg-surface-4 p-2 text-sm"
								>
									<button
										v-tooltip.left="
											process.uuid === selectedProcess.uuid
												? formatMessage(messages.primaryInstance)
												: formatMessage(messages.makePrimaryInstance)
										"
										class="flex flex-grow items-center gap-2"
										:class="{
											'active:scale-95 transition-transform': process.uuid !== selectedProcess.uuid,
										}"
										:disabled="process.uuid === selectedProcess.uuid"
										@click="selectProcess(process)"
									>
										<OnlineIndicatorIcon />
										<span class="mr-auto text-contrast flex items-center gap-2">
											{{ process.instance.name }}
											<StarIcon v-if="process.uuid === selectedProcess.uuid" class="text-orange" />
										</span>
									</button>
									<button
										v-tooltip="formatMessage(messages.stopInstance)"
										class="active:scale-95 flex"
										:disabled="stopping.includes(process.uuid)"
										:aria-label="formatMessage(messages.stopInstance)"
										@click.stop="stop(process)"
									>
										<StopCircleIcon class="text-red size-5" />
									</button>
									<button
										v-tooltip="formatMessage(messages.viewLogs)"
										class="active:scale-95 flex"
										@click.stop="goToTerminal(process)"
									>
										<TerminalSquareIcon class="text-secondary size-5" />
									</button>
								</div>
							</div>
						</template>
					</FloatingMenu>
				</div>
				<button
					v-tooltip="formatMessage(messages.stopInstance)"
					class="active:scale-95 flex"
					:disabled="stopping.includes(selectedProcess.uuid)"
					:aria-label="formatMessage(messages.stopInstance)"
					@click="stop(selectedProcess)"
				>
					<StopCircleIcon class="text-red size-5" />
				</button>
				<button
					v-tooltip="formatMessage(messages.viewLogs)"
					class="active:scale-95 flex"
					@click="goToTerminal()"
				>
					<TerminalSquareIcon class="text-secondary size-5" />
				</button>
			</template>
			<template v-else>
				<span class="size-2 rounded-full bg-secondary" />
				<span class="text-secondary"> {{ formatMessage(messages.noInstancesRunning) }} </span>
			</template>
		</div>
	</div>
</template>

<script setup lang="ts">
import {
	DownloadIcon,
	DropdownIcon,
	OnlineIndicatorIcon,
	StarIcon,
	StopCircleIcon,
	TerminalSquareIcon,
	UnplugIcon,
} from '@orbiont/assets'
import {
	defineMessages,
	FloatingMenu,
	IconButton,
	injectNotificationManager,
	injectPopupNotificationManager,
	type PopupNotificationProgressItem,
	type PopupNotificationStandard,
	useVIntl,
} from '@orbiont/ui'
import { useQuery } from '@tanstack/vue-query'
import { convertFileSrc } from '@tauri-apps/api/core'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import AppUpdateButton from '@/components/ui/app-update-button/index.vue'
import DownloadManager from '@/components/ui/download-manager/index.vue'
import { useAppEvent } from '@/composables/use-app-event'
import { bedrockStatusQueryOptions, stopBedrock } from '@/helpers/bedrock'
import { bedrockMessages } from '@/helpers/bedrock-messages'
import { get_many as getInstances } from '@/helpers/instance'
import { get_all as getRunningProcesses, kill as killProcess } from '@/helpers/process'
import type { LoadingBar } from '@/helpers/state'
import { progress_bars_list } from '@/helpers/state'
import type { GameInstance } from '@/helpers/types'

const { handleError } = injectNotificationManager()
const popupNotificationManager = injectPopupNotificationManager()
const { formatMessage } = useVIntl()

const router = useRouter()

const showInstances = ref(false)

interface RunningProcess {
	uuid: string
	instance_id: string
	instance: Pick<GameInstance, 'id' | 'name'>
	edition: 'java' | 'bedrock'
}

const messages = defineMessages({
	offline: {
		id: 'app.action-bar.offline',
		defaultMessage: 'Offline',
	},
	viewInstance: {
		id: 'app.action-bar.view-instance',
		defaultMessage: 'View instance',
	},
	showMoreRunningInstances: {
		id: 'app.action-bar.show-more-running-instances',
		defaultMessage: 'Show more running instances',
	},
	hideMoreRunningInstances: {
		id: 'app.action-bar.hide-more-running-instances',
		defaultMessage: 'Hide more running instances',
	},
	primaryInstance: {
		id: 'app.action-bar.primary-instance',
		defaultMessage: 'Primary instance',
	},
	makePrimaryInstance: {
		id: 'app.action-bar.make-primary-instance',
		defaultMessage: 'Make primary instance',
	},
	stopInstance: {
		id: 'app.action-bar.stop-instance',
		defaultMessage: 'Stop instance',
	},
	viewLogs: {
		id: 'app.action-bar.view-logs',
		defaultMessage: 'View logs',
	},
	noInstancesRunning: {
		id: 'app.action-bar.no-instances-running',
		defaultMessage: 'No instances running',
	},
	downloadingJava: {
		id: 'app.action-bar.downloading-java',
		defaultMessage: 'Downloading Java {version}',
	},
	downloads: {
		id: 'app.action-bar.downloads',
		defaultMessage: 'Downloads',
	},
	viewActiveDownloads: {
		id: 'app.action-bar.view-active-downloads',
		defaultMessage: 'View active downloads',
	},
	hideDownloads: {
		id: 'app.action-bar.hide-downloads',
		defaultMessage: 'Hide active downloads',
	},
	showDownloads: {
		id: 'app.action-bar.show-downloads',
		defaultMessage: 'Show active downloads',
	},
})

const downloadState = computed(() => popupNotificationManager.getDownloadState())
const downloadToggleLabel = computed(() =>
	formatMessage(
		downloadState.value.hidden > 0
			? messages.showDownloads
			: downloadState.value.total > 0
				? messages.hideDownloads
				: messages.viewActiveDownloads,
	),
)

function toggleDownloadNotifications(): void {
	if (downloadState.value.total > 0) {
		popupNotificationManager.toggleDownloadNotifications()
	} else if (hasActiveLoadingBars.value) {
		openDownloadToast()
	}
}

const javaProcesses = ref<RunningProcess[]>([])
const bedrockStatus = useQuery(bedrockStatusQueryOptions())
const currentProcesses = computed<RunningProcess[]>(() => [
	...javaProcesses.value,
	...(bedrockStatus.data.value?.game_running
		? [
				{
					uuid: 'bedrock',
					instance_id: 'bedrock',
					instance: { id: 'bedrock', name: formatMessage(bedrockMessages.title) },
					edition: 'bedrock' as const,
				},
			]
		: []),
])
const selectedUuid = ref<string>()
const selectedProcess = computed(
	() =>
		currentProcesses.value.find((process) => process.uuid === selectedUuid.value) ??
		currentProcesses.value[0],
)
const stopping = ref<string[]>([])

const refresh = async () => {
	const processes = ((await getRunningProcesses().catch((error) => {
		handleError(error)
		return []
	})) ?? []) as Array<{ uuid: string; instance_id: string }>
	const instanceIds = processes.map((process) => process.instance_id)
	const instances: GameInstance[] = await getInstances(instanceIds).catch((error) => {
		handleError(error)
		return []
	})

	javaProcesses.value = processes
		.map((process) => {
			const instance = instances.find((item) => process.instance_id === item.id)
			if (!instance) {
				return null
			}
			return {
				...process,
				instance,
				edition: 'java' as const,
			}
		})
		.filter((process): process is RunningProcess => process !== null)
}

await refresh()

const offline = ref(!navigator.onLine)
function handleOffline() {
	offline.value = true
}
function handleOnline() {
	offline.value = false
}

onMounted(() => {
	window.addEventListener('offline', handleOffline)
	window.addEventListener('online', handleOnline)
})

useAppEvent('process', async () => {
	await refresh()
})

const stop = async (process: RunningProcess) => {
	if (stopping.value.includes(process.uuid)) return
	stopping.value.push(process.uuid)
	try {
		if (process.edition === 'bedrock') {
			await stopBedrock()
			await bedrockStatus.refetch()
		} else {
			await killProcess(process.uuid)
			await refresh()
		}
	} catch (e) {
		handleError(e)
	} finally {
		stopping.value = stopping.value.filter((uuid) => uuid !== process.uuid)
	}
}

function processRoute(process: RunningProcess, logs = false) {
	return process.edition === 'bedrock'
		? logs
			? '/bedrock?tab=logs'
			: '/bedrock'
		: `/instance/${encodeURIComponent(process.instance.id)}${logs ? '/logs' : ''}`
}

function goToTerminal(process = selectedProcess.value) {
	if (process) void router.push(processRoute(process, true))
}

const currentLoadingBars = ref<LoadingBar[]>([])
const currentLoadingBarIconUrls = ref<Record<string, string | null>>({})
const notificationId = ref<string | number | null>(null)
const dismissed = ref(false)

function getLoadingBarKey(loadingBar: LoadingBar): string {
	return `${loadingBar.loading_bar_uuid ?? loadingBar.id}`
}

function getLoadingProgress(loadingBar: LoadingBar): number {
	if (!loadingBar.total || loadingBar.total <= 0) {
		return 0
	}
	return Math.max(0, Math.min(1, (loadingBar.current ?? 0) / (loadingBar.total ?? 0)))
}

function getLoadingText(loadingBar: LoadingBar): string {
	return loadingBar.message ?? ''
}

function getDisplayIconUrl(icon: string | null | undefined): string | null {
	if (!icon) {
		return null
	}
	if (/^(https?:|data:|blob:|asset:|tauri:)/.test(icon)) {
		return icon
	}
	return convertFileSrc(icon)
}

function getNotification(): PopupNotificationStandard | null {
	if (!notificationId.value) {
		return null
	}
	const notification = popupNotificationManager
		.getNotifications()
		.find((notification) => notification.id === notificationId.value)
	return notification?.contentType === 'standard' && notification.type === 'download'
		? notification
		: null
}

function removeNotification(): void {
	if (!notificationId.value) {
		return
	}
	popupNotificationManager.removeNotification(notificationId.value)
	notificationId.value = null
}

function buildDownloadItems(): PopupNotificationProgressItem[] {
	return currentLoadingBars.value.map((bar) => ({
		id: getLoadingBarKey(bar),
		title: bar.title ?? '',
		text: getLoadingText(bar),
		iconUrl: currentLoadingBarIconUrls.value[getLoadingBarKey(bar)] ?? null,
		progress: getLoadingProgress(bar),
		waiting: !bar.total || bar.total <= 0,
		progressType: bar.bar_type?.type === 'pack_import' ? 'bytes' : 'percentage',
		progressCurrent: bar.current,
		progressTotal: bar.total,
	}))
}

const hasActiveLoadingBars = computed(() => currentLoadingBars.value.length > 0)

function updateNotification(resummon = false): void {
	if (resummon) {
		dismissed.value = false
	}

	if (currentLoadingBars.value.length === 0) {
		removeNotification()
		dismissed.value = false
		return
	}

	if (notificationId.value && !getNotification()) {
		notificationId.value = null
		dismissed.value = true
	}

	if (dismissed.value && !resummon) {
		return
	}

	const notif = getNotification()
	const progressItems = buildDownloadItems()

	if (notif) {
		notif.title = formatMessage(messages.downloads)
		notif.text = undefined
		notif.progressItems = progressItems
		notif.progress = undefined
		notif.waiting = undefined
	} else {
		const notification = popupNotificationManager.addPopupNotification({
			contentType: 'standard',
			title: formatMessage(messages.downloads),
			type: 'download',
			autoCloseMs: null,
			progressItems,
		})
		notificationId.value = notification.id
	}
}

function formatLoadingBars(loadingBar: LoadingBar): LoadingBar {
	const formatted = { ...loadingBar }
	if (formatted.bar_type?.type === 'java_download') {
		formatted.title = formatMessage(messages.downloadingJava, {
			version: formatted.bar_type.version,
		})
	}
	if (formatted.bar_type?.instance_id) {
		formatted.title = formatted.bar_type.instance_name ?? formatted.bar_type.instance_id
	}
	if (formatted.bar_type?.pack_name) {
		formatted.title = formatted.bar_type.pack_name
	}
	return formatted
}

async function refreshLoadingBars() {
	const bars: Record<string, LoadingBar> = await progress_bars_list().catch((error) => {
		handleError(error)
		return {}
	})

	currentLoadingBars.value = Object.values(bars)
		.map(formatLoadingBars)
		.filter(
			(bar) =>
				bar?.bar_type?.type !== 'launcher_update' &&
				![
					'java_download',
					'pack_file_download',
					'pack_download',
					'minecraft_download',
					'copy_instance',
				].includes(bar?.bar_type?.type ?? ''),
		)

	const instanceIds = Array.from(
		new Set(
			currentLoadingBars.value
				.map((bar) => bar.bar_type?.instance_id)
				.filter((instanceId): instanceId is string => !!instanceId),
		),
	)
	const instances = instanceIds.length
		? await getInstances(instanceIds).catch((error) => {
				handleError(error)
				return []
			})
		: []
	const instanceIconUrls = new Map(
		instances.map((instance) => [instance.id, getDisplayIconUrl(instance.icon_path)]),
	)
	currentLoadingBarIconUrls.value = Object.fromEntries(
		currentLoadingBars.value.map((bar) => {
			const barIconUrl = getDisplayIconUrl(bar.bar_type?.icon)
			const instanceIconUrl = bar.bar_type?.instance_id
				? instanceIconUrls.get(bar.bar_type.instance_id)
				: null
			return [getLoadingBarKey(bar), barIconUrl ?? instanceIconUrl ?? null]
		}),
	)

	currentLoadingBars.value.sort((a, b) => {
		const aKey = `${a.loading_bar_uuid ?? a.id ?? ''}`
		const bKey = `${b.loading_bar_uuid ?? b.id ?? ''}`
		return aKey.localeCompare(bKey)
	})

	updateNotification()
}

await refreshLoadingBars()

useAppEvent('loading', async () => {
	await refreshLoadingBars()
})

function openDownloadToast() {
	updateNotification(true)
}

function selectProcess(process: RunningProcess) {
	selectedUuid.value = process.uuid
}

onBeforeUnmount(() => {
	removeNotification()
	dismissed.value = false
	window.removeEventListener('offline', handleOffline)
	window.removeEventListener('online', handleOnline)
})
</script>
