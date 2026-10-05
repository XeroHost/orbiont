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
import { useQuery } from '@tanstack/vue-query'
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

import BedrockInstalled from '@/components/ui/bedrock/BedrockInstalled.vue'
import {
	bedrockStatusQueryOptions,
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
const busy = ref(false)
const actionError = ref('')
const submitted = ref(false)
const updateRequested = ref(false)
const canPlay = computed(
	() =>
		!!installation.value?.supported &&
		!!installation.value.game?.can_launch &&
		!status.isError.value,
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
		await status.refetch()
	} catch (error) {
		const code = (error as { code?: string })?.code
		const message =
			code === 'invalid_file'
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
			:running="installation.game_running ?? false"
			@play="action(launchBedrock, messages.launchError)"
			@stop="action(stopBedrock, commonMessages.errorNotificationTitle)"
			@update="updateMinecraft"
			@launcher="action(launchOfficialMinecraftLauncher, messages.launchError)"
			@refresh="status.refetch()"
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
							color="green"
							:disabled="busy || !canPlay"
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
						<Button
							type="outlined"
							:disabled="busy || status.isFetching.value"
							@click="status.refetch()"
						>
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
					<Button type="outlined" :disabled="busy || !canPlay" @click="importFile">
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
					><AnimatedIcon name="puzzle" />{{ formatMessage(bedrockCatalogMessages.addons) }}</Button
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
		<Admonition v-if="updateRequested" type="info">{{
			formatMessage(messages.updateHelp)
		}}</Admonition>
		<Admonition v-if="submitted" type="success" role="status">{{
			formatMessage(messages.submitted)
		}}</Admonition>
		<Admonition v-if="actionError" type="warning" role="alert">{{ actionError }}</Admonition>
	</main>
</template>
