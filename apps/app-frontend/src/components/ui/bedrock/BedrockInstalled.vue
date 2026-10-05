<script setup lang="ts">
import {
	BoxesIcon,
	DownloadIcon,
	ExternalIcon,
	FolderOpenIcon,
	GlobeIcon,
	HistoryIcon,
	MoreVerticalIcon,
	StopCircleIcon,
	TerminalSquareIcon,
	UploadIcon,
} from '@orbiont/assets'
import {
	Admonition,
	AnimatedIcon,
	Button,
	commonMessages,
	NavTabs,
	TeleportOverflowMenu,
	useVIntl,
} from '@orbiont/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'
import { onBeforeRouteLeave, useRoute } from 'vue-router'

import bedrockLogo from '@/assets/editions/bedrock-edition.png?url'
import {
	type BedrockApplication,
	type BedrockItem,
	deleteBedrockItem,
	getBedrockWorkspace,
	openBedrockFolder,
} from '@/helpers/bedrock'
import { bedrockMessages as messages } from '@/helpers/bedrock-messages'

import BedrockContent from './BedrockContent.vue'
import BedrockFiles from './BedrockFiles.vue'
import BedrockItems from './BedrockItems.vue'
import BedrockLogs from './BedrockLogs.vue'
import BedrockManagement from './BedrockManagement.vue'

const props = defineProps<{
	game: BedrockApplication
	launcherAvailable: boolean
	busy: boolean
	running: boolean
}>()
const emit = defineEmits<{
	play: []
	stop: []
	update: []
	launcher: []
	refresh: []
	import: []
}>()
const { formatMessage } = useVIntl()
const queryClient = useQueryClient()
const activeTab = ref(0)
const route = useRoute()
const folderBusy = ref(false)
const folderError = ref(false)
const mutationBusy = ref(false)
const mutationError = ref('')
const management = ref<InstanceType<typeof BedrockManagement>>()
const fileView = ref<InstanceType<typeof BedrockFiles>>()
const manageBusy = computed(
	() => props.busy || folderBusy.value || mutationBusy.value || props.running,
)
async function changeTab(index: number) {
	if (activeTab.value === 1 && !(await fileView.value?.confirmLeave())) return
	activeTab.value = index
}
watch(
	() => route.query.tab,
	(tab) => {
		if (tab === 'logs') void changeTab(3)
	},
	{ immediate: true },
)
onBeforeRouteLeave(
	async () => activeTab.value !== 1 || (await fileView.value?.confirmLeave()) !== false,
)
function confirmDelete(items: BedrockItem[]) {
	return management.value?.confirm(items) ?? Promise.resolve(false)
}
async function deleteItem(item: BedrockItem) {
	if (props.running || mutationBusy.value) {
		mutationError.value = formatMessage(messages.closeToManage)
		throw new Error(mutationError.value)
	}
	mutationBusy.value = true
	mutationError.value = ''
	try {
		await deleteBedrockItem(item.root_id, item.path)
		await queryClient.invalidateQueries({ queryKey: ['bedrock'] })
	} catch (error) {
		mutationError.value = formatMessage(
			(error as { code?: string })?.code === 'conflict'
				? messages.conflictError
				: messages.mutationError,
		)
		throw new Error(mutationError.value, { cause: error })
	} finally {
		mutationBusy.value = false
	}
}
async function deleteWorld(items: BedrockItem[]) {
	if (!(await confirmDelete(items))) return
	try {
		for (const item of items) await deleteItem(item)
	} catch {
		/* The workspace keeps the failure visible. */
	}
}
const workspace = useQuery({
	queryKey: ['bedrock', 'workspace'],
	queryFn: getBedrockWorkspace,
	staleTime: 5000,
	refetchOnWindowFocus: 'always',
	refetchInterval: 15_000,
	retry: false,
})
const data = workspace.data
const tabs = computed(() => [
	{ href: 'content', label: formatMessage(messages.content), icon: BoxesIcon },
	{ href: 'files', label: formatMessage(messages.files), icon: FolderOpenIcon },
	{ href: 'worlds', label: formatMessage(messages.worlds), icon: GlobeIcon },
	{ href: 'logs', label: formatMessage(messages.logs), icon: TerminalSquareIcon },
])
const items = computed(() =>
	(data.value?.items ?? []).filter((item) =>
		activeTab.value === 2
			? item.kind === 'world'
			: ['resource_pack', 'skin_pack', 'behavior_pack'].includes(item.kind),
	),
)
const emptyMessage = computed(() =>
	formatMessage(activeTab.value === 2 ? messages.noWorlds : messages.noContent),
)
async function refresh() {
	emit('refresh')
	await queryClient.invalidateQueries({ queryKey: ['bedrock'] })
}
async function openFolder(rootId: string, path: string) {
	if (folderBusy.value) return
	folderBusy.value = true
	folderError.value = false
	try {
		await openBedrockFolder(rootId, path)
	} catch {
		folderError.value = true
	} finally {
		folderBusy.value = false
	}
}
</script>

<template>
	<div class="flex min-w-0 flex-col gap-4">
		<BedrockManagement ref="management" :running="running" />
		<header
			class="flex flex-wrap items-center justify-between gap-4 border-0 border-b border-solid border-surface-4 pb-4"
		>
			<div class="flex min-w-0 items-center gap-4">
				<img
					:src="bedrockLogo"
					alt=""
					class="size-16 shrink-0 rounded-xl object-contain [image-rendering:pixelated]"
				/>
				<div>
					<h1 class="m-0 text-2xl font-bold text-contrast">{{ formatMessage(messages.title) }}</h1>
					<p class="mb-0 mt-1 text-secondary">
						{{ formatMessage(messages.installation) }} ·
						{{ formatMessage(messages.version, { version: game.version }) }}
					</p>
				</div>
			</div>
			<div class="flex flex-wrap gap-3">
				<Button
					type="colored"
					:color="running ? 'red' : 'brand'"
					size="xl"
					:disabled="busy"
					@click="running ? emit('stop') : emit('play')"
					><StopCircleIcon v-if="running" /><AnimatedIcon v-else name="play" />{{
						formatMessage(running ? commonMessages.stopButton : messages.play)
					}}</Button
				>
				<TeleportOverflowMenu
					type="quiet"
					size="xl"
					:label="formatMessage(commonMessages.moreOptionsButton)"
					:tooltip="formatMessage(commonMessages.moreOptionsButton)"
					:options="[
						{
							id: 'update',
							label: formatMessage(messages.update),
							icon: DownloadIcon,
							action: () => emit('update'),
							disabled: busy || running,
						},
						{
							id: 'recoveries',
							label: formatMessage(messages.recoveryTitle),
							icon: HistoryIcon,
							action: () => management?.showRecovery(),
						},
						{
							id: 'launcher',
							label: formatMessage(messages.launcher),
							icon: ExternalIcon,
							action: () => emit('launcher'),
							disabled: busy || !launcherAvailable,
						},
					]"
				>
					<MoreVerticalIcon />
				</TeleportOverflowMenu>
			</div>
		</header>
		<div class="overflow-x-auto">
			<NavTabs
				class="card-shadow border border-solid border-surface-4"
				:links="tabs"
				mode="local"
				:active-index="activeTab"
				@tab-click="changeTab"
			/>
		</div>
		<Admonition v-if="running" type="info">{{ formatMessage(messages.closeToManage) }}</Admonition>
		<Admonition v-if="mutationError" type="warning" role="alert">{{ mutationError }}</Admonition>
		<div v-if="activeTab === 2 || activeTab === 3" class="flex flex-wrap items-center gap-3">
			<Button v-if="activeTab === 2" type="outlined" :disabled="busy" @click="emit('import')"
				><UploadIcon />{{ formatMessage(messages.importButton) }}</Button
			>
			<Button type="outlined" :disabled="busy || workspace.isFetching.value" @click="refresh"
				><AnimatedIcon name="refresh" />{{ formatMessage(messages.refreshData) }}</Button
			>
		</div>
		<p v-if="workspace.isPending.value" role="status">{{ formatMessage(messages.loadingData) }}</p>
		<Admonition v-else-if="workspace.isError.value" type="warning" role="alert">{{
			formatMessage(messages.dataError)
		}}</Admonition>
		<template v-else-if="data">
			<Admonition v-if="data.incomplete" type="warning">{{
				formatMessage(messages.partialData)
			}}</Admonition>
			<Admonition v-if="!data.roots.some((root) => root.kind !== 'logs')" type="info">{{
				formatMessage(messages.noData)
			}}</Admonition>
			<BedrockFiles
				v-if="activeTab === 1"
				ref="fileView"
				:roots="data.roots"
				:running="running"
				:busy="busy || folderBusy || mutationBusy"
				@open-folder="openFolder"
			/>
			<BedrockLogs
				v-else-if="activeTab === 3"
				:items="data.items"
				:roots="data.roots"
				:busy="busy || folderBusy"
				@open-folder="openFolder"
			/>
			<BedrockContent
				v-else-if="activeTab === 0"
				:items="items"
				:worlds="data.items.filter((item) => item.kind === 'world')"
				:busy="manageBusy"
				:confirm-delete="confirmDelete"
				:delete-item="deleteItem"
				@open-folder="openFolder"
				@import="emit('import')"
				@refresh="refresh"
			/>
			<BedrockItems
				v-else
				:key="activeTab"
				:items="items"
				:empty-message="emptyMessage"
				:busy="manageBusy"
				@delete="deleteWorld"
				@open-folder="openFolder"
			/>
		</template>
		<Admonition v-if="folderError" type="warning" role="alert">{{
			formatMessage(messages.folderError)
		}}</Admonition>
	</div>
</template>
