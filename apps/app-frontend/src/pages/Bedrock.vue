<script setup lang="ts">
import { DownloadIcon, ExternalIcon, PlayIcon, UploadIcon } from '@orbiont/assets'
import {
	Admonition,
	AnimatedIcon,
	Button,
	commonMessages,
	commonProjectTypeCategoryMessages,
	useVIntl,
} from '@orbiont/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

import BedrockDiagnostics from '@/components/ui/bedrock/BedrockDiagnostics.vue'
import BedrockInstalled from '@/components/ui/bedrock/BedrockInstalled.vue'
import BedrockStopConfirm from '@/components/ui/bedrock/BedrockStopConfirm.vue'
import {
	bedrockStatusQueryOptions,
	getBedrockProcessStatus,
	getBedrockStatus,
	isBedrockLaunchPending,
	launchBedrock,
	launchOfficialMinecraftLauncher,
	openBedrockStore,
	openBedrockUpdates,
	pickAndImportBedrockFile,
	stopBedrock,
} from '@/helpers/bedrock'
import { bedrockCatalogMessages } from '@/helpers/bedrock-catalog-messages'
import { bedrockMessages as messages } from '@/helpers/bedrock-messages'
import { useRootBreadcrumb } from '@/providers/breadcrumbs'

const { formatMessage } = useVIntl()
const router = useRouter()
useRootBreadcrumb({
	slot: 'root',
	id: 'bedrock',
	label: formatMessage(messages.title),
	to: '/bedrock',
	visual: { type: 'icon', component: PlayIcon },
})

const status = useQuery(bedrockStatusQueryOptions())
const installation = status.data
const queryClient = useQueryClient()
const processStatus = useQuery({
	queryKey: ['bedrock', 'process'],
	queryFn: getBedrockProcessStatus,
	enabled: computed(() => !!installation.value?.supported),
	refetchInterval: 2000,
	refetchOnWindowFocus: 'always',
	retry: false,
})
const running = computed(
	() => processStatus.data.value?.game_running ?? installation.value?.game_running ?? false,
)
const launchPending = computed(() =>
	isBedrockLaunchPending(processStatus.data.value?.launch ?? installation.value?.launch),
)
const diagnosticStatus = computed(() =>
	installation.value ? { ...installation.value, ...processStatus.data.value } : null,
)
const refreshing = ref(false)
async function refreshStatus() {
	if (refreshing.value || busy.value) return
	refreshing.value = true
	try {
		await Promise.allSettled([
			queryClient.fetchQuery({
				queryKey: ['bedrock', 'status'],
				queryFn: () => getBedrockStatus(true),
				staleTime: 0,
			}),
			processStatus.refetch({ cancelRefetch: false }),
		])
	} finally {
		refreshing.value = false
	}
}
const busy = ref(false)
const stopConfirm = ref<InstanceType<typeof BedrockStopConfirm>>()
function stopMinecraft() {
	return stopBedrock(() => stopConfirm.value?.ask() ?? Promise.resolve(false))
}
const actionError = ref('')
const submitted = ref(false)
const updateRequested = ref(false)
const canPlay = computed(
	() => !!installation.value?.supported && !!installation.value.game?.can_launch,
)
const canOpenLauncher = computed(
	() => !!installation.value?.launcher?.can_launch && !status.isError.value,
)

async function action(
	operation: () => Promise<unknown>,
	fallback: (typeof messages)[keyof typeof messages],
) {
	if (busy.value) return
	busy.value = true
	actionError.value = ''
	submitted.value = false
	try {
		await operation()
		await processStatus.refetch()
		await status.refetch()
		await queryClient.invalidateQueries({ queryKey: ['bedrock', 'workspace'] })
	} catch (error) {
		const code = (error as { code?: string })?.code
		const message =
			code === 'insufficient_space'
				? messages.insufficientSpace
				: code === 'game_running'
					? messages.closeToManage
					: code === 'invalid_file'
						? messages.invalidFile
						: code === 'file_too_large'
							? messages.largeFile
							: code === 'not_installed'
								? messages.missing
								: code === 'needs_repair'
									? messages.repair
									: fallback
		actionError.value = formatMessage(message)
		void status.refetch()
	} finally {
		busy.value = false
	}
}
async function importFile() {
	if (running.value || launchPending.value) return
	await action(async () => {
		submitted.value = await pickAndImportBedrockFile(formatMessage(messages.fileFilter))
	}, messages.importError)
}
async function updateMinecraft() {
	updateRequested.value = true
	await action(openBedrockUpdates, messages.storeError)
}
</script>

<template>
	<main
		:aria-busy="busy || status.isPending.value"
		class="flex w-full flex-col gap-6 p-6"
		:class="{ 'mx-auto max-w-4xl': !canPlay }"
	>
		<BedrockInstalled
			v-if="canPlay && installation?.game"
			:game="installation.game"
			:launcher-available="canOpenLauncher"
			:busy="busy"
			:running="running"
			:launch-pending="launchPending"
			@play="action(launchBedrock, messages.launchError)"
			@stop="action(stopMinecraft, commonMessages.errorNotificationTitle)"
			@update="updateMinecraft"
			@launcher="action(launchOfficialMinecraftLauncher, messages.launchError)"
			@refresh="refreshStatus"
			@import="importFile"
		/>
		<template v-else>
			<header>
				<h1 class="m-0 text-3xl font-bold text-contrast">{{ formatMessage(messages.title) }}</h1>
				<p class="mb-0 text-secondary">{{ formatMessage(messages.description) }}</p>
			</header>
			<p v-if="status.isPending.value" role="status">{{ formatMessage(messages.checking) }}</p>
			<Admonition v-else-if="installation && !installation.supported" type="info">
				{{ formatMessage(messages.unsupported) }}
			</Admonition>
			<template v-else>
				<Admonition v-if="status.isError.value" type="warning" role="alert">
					{{ formatMessage(messages.detectionError) }}
				</Admonition>
				<section class="flex flex-col gap-4 rounded-xl bg-surface-3 p-5">
					<h2 v-if="!status.isError.value" class="m-0 text-xl font-semibold text-contrast">
						{{ formatMessage(installation?.game ? messages.installed : messages.missing) }}
					</h2>
					<p v-if="installation?.game && !installation.game.can_launch" class="m-0 text-secondary">
						{{ formatMessage(messages.repair) }}
					</p>
					<p v-else-if="canOpenLauncher && !installation?.game" class="m-0 text-secondary">
						{{ formatMessage(messages.launcherDetected) }}
					</p>
					<div class="flex flex-wrap gap-3">
						<Button
							type="colored"
							color="brand"
							size="xl"
							:disabled="busy || running || launchPending || !canPlay"
							@click="action(launchBedrock, messages.launchError)"
						>
							<AnimatedIcon name="play" />{{ formatMessage(messages.play) }}
						</Button>
						<Button
							type="outlined"
							:disabled="busy || !canOpenLauncher"
							@click="action(launchOfficialMinecraftLauncher, messages.launchError)"
						>
							<ExternalIcon />{{ formatMessage(messages.launcher) }}
						</Button>
						<Button
							type="outlined"
							:disabled="busy || status.isPending.value"
							@click="action(openBedrockStore, messages.storeError)"
						>
							<DownloadIcon />{{ formatMessage(messages.store) }}
						</Button>
						<Button type="outlined" :disabled="busy" :loading="refreshing" @click="refreshStatus">
							<AnimatedIcon name="refresh" />{{ formatMessage(messages.refresh) }}
						</Button>
					</div>
				</section>
				<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.account) }}</p>
				<section class="flex flex-col items-start gap-3 rounded-xl bg-surface-3 p-5">
					<h2 class="m-0 text-xl font-semibold text-contrast">
						{{ formatMessage(messages.importTitle) }}
					</h2>
					<p class="m-0 text-secondary">{{ formatMessage(messages.importDescription) }}</p>
					<Button
						type="outlined"
						:disabled="busy || running || launchPending || !canPlay"
						@click="importFile"
					>
						<UploadIcon />{{ formatMessage(messages.importButton) }}
					</Button>
				</section>
			</template>
		</template>
		<section v-if="!canPlay" class="flex flex-col gap-3 rounded-xl bg-surface-3 p-5">
			<h2 class="m-0 text-xl font-semibold text-contrast">
				{{ formatMessage(bedrockCatalogMessages.discover) }}
			</h2>
			<p class="m-0 text-secondary">{{ formatMessage(bedrockCatalogMessages.description) }}</p>
			<div class="flex flex-wrap gap-3">
				<Button type="outlined" @click="router.push('/browse/mod?edition=bedrock&src=curseforge')"
					><AnimatedIcon name="cube" />{{ formatMessage(bedrockCatalogMessages.addons) }}</Button
				>
				<Button
					type="outlined"
					@click="router.push('/browse/resourcepack?edition=bedrock&src=curseforge')"
					><AnimatedIcon name="image" />{{
						formatMessage(commonProjectTypeCategoryMessages.resourcepack)
					}}</Button
				>
				<Button type="outlined" @click="router.push('/browse/world?edition=bedrock&src=curseforge')"
					><AnimatedIcon name="globe" />{{
						formatMessage(commonProjectTypeCategoryMessages.world)
					}}</Button
				>
				<Button
					type="outlined"
					@click="router.push('/browse/datapack?edition=bedrock&src=curseforge')"
					><AnimatedIcon name="terminal" />{{
						formatMessage(bedrockCatalogMessages.scripts)
					}}</Button
				>
			</div>
		</section>
		<BedrockDiagnostics
			v-if="diagnosticStatus"
			:status="diagnosticStatus"
			:busy="busy"
			:refreshing="refreshing"
			:failed="processStatus.isError.value || status.isError.value"
			@launcher="action(launchOfficialMinecraftLauncher, messages.launchError)"
			@store="action(openBedrockStore, messages.storeError)"
			@refresh="refreshStatus"
		/>
		<Admonition v-if="updateRequested" type="info">{{
			formatMessage(messages.updateHelp)
		}}</Admonition>
		<Admonition v-if="submitted" type="success" role="status">{{
			formatMessage(messages.submitted)
		}}</Admonition>
		<Admonition v-if="actionError" type="warning" role="alert">{{ actionError }}</Admonition>
		<BedrockStopConfirm ref="stopConfirm" />
	</main>
</template>
