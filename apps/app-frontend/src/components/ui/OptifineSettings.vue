<template>
	<section class="mt-5 flex flex-col gap-3 rounded-xl border border-solid border-surface-5 p-4">
		<span class="font-semibold text-contrast">OptiFine</span>
		<span v-if="reference" class="text-sm text-secondary">{{
			formatMessage(optifineMessages.version, {
				version: reference.version,
				gameVersion: reference.minecraftVersion,
			})
		}}</span>
		<div class="flex flex-wrap gap-2">
			<Button
				v-if="gameLoader === 'vanilla'"
				type="outlined"
				color="green"
				:disabled="disabled || busy || status.isPending.value || status.isError.value"
				@click="install"
				><UploadIcon />{{ formatMessage(optifineMessages.add) }}</Button
			>
			<Button v-if="reference" type="outlined" :disabled="disabled || busy" @click="remove">{{
				formatMessage(optifineMessages.remove)
			}}</Button>
			<Button type="outlined" @click="download"
				><DownloadIcon />{{ formatMessage(optifineMessages.download) }}</Button
			>
			<Button v-if="status.isError.value" type="outlined" @click="status.refetch()">{{
				formatMessage(commonMessages.retryButton)
			}}</Button>
		</div>
	</section>
</template>

<script setup lang="ts">
import { DownloadIcon, UploadIcon } from '@orbiont/assets'
import { Button, commonMessages, injectNotificationManager, useVIntl } from '@orbiont/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import { wait_for_install_job } from '@/helpers/install'
import {
	changeInstanceOptifine,
	getInstanceOptifine,
	openOptifineDownloads,
	optifineMessages,
	pickOptifineInstaller,
} from '@/helpers/optifine'
import { injectAppEvents } from '@/providers/app-events'

const props = defineProps<{
	instanceId: string
	gameVersion: string
	gameLoader: string
	disabled: boolean
}>()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const appEvents = injectAppEvents()
const busy = ref(false)
const status = useQuery({
	queryKey: computed(() => ['optifine', 'instance', props.instanceId] as const),
	queryFn: ({ queryKey }) => getInstanceOptifine(queryKey[2]),
	staleTime: 0,
})
const reference = status.data
async function download() {
	try {
		await openOptifineDownloads()
	} catch (error) {
		handleError(error as Error)
	}
}
async function change(path: string | null) {
	const job = await changeInstanceOptifine(props.instanceId, path)
	await wait_for_install_job(appEvents, job.job_id)
	await status.refetch()
}
async function install() {
	busy.value = true
	try {
		const selected = await pickOptifineInstaller(props.gameVersion)
		if (selected) await change(selected.path)
	} catch (error) {
		handleError(error as Error)
	} finally {
		busy.value = false
		void status.refetch()
	}
}
async function remove() {
	busy.value = true
	try {
		await change(null)
	} catch (error) {
		handleError(error as Error)
	} finally {
		busy.value = false
		void status.refetch()
	}
}
</script>
