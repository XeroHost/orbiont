<template>
	<NewModal
		ref="modal"
		:header="formatMessage(optifineMessages.importTitle)"
		@hide="settle(null, false)"
	>
		<div class="flex max-w-[480px] flex-col gap-4 p-5">
			<p v-if="reference" class="m-0 text-secondary">
				{{
					formatMessage(optifineMessages.importDescription, {
						version: reference.version,
						gameVersion: reference.minecraftVersion,
					})
				}}
			</p>
			<div class="flex flex-wrap gap-2">
				<Button type="outlined" @click="download"
					><DownloadIcon />{{ formatMessage(optifineMessages.download) }}</Button
				>
				<Button type="colored" color="green" :disabled="busy" @click="choose"
					><UploadIcon />{{ formatMessage(optifineMessages.continue) }}</Button
				>
				<Button type="outlined" @click="cancel">{{
					formatMessage(commonMessages.cancelButton)
				}}</Button>
			</div>
		</div>
	</NewModal>
</template>

<script setup lang="ts">
import { DownloadIcon, UploadIcon } from '@orbiont/assets'
import { Button, commonMessages, injectNotificationManager, NewModal, useVIntl } from '@orbiont/ui'
import { ref, useTemplateRef } from 'vue'

import { openOptifineDownloads, optifineMessages, pickOptifineInstaller } from '@/helpers/optifine'
import type { OptifineReference } from '@/helpers/optifine-selection'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')
const reference = ref<OptifineReference | null>(null)
const busy = ref(false)
let resolve: ((value: string | null) => void) | null = null
let generation = 0
function settle(path: string | null, hide = true) {
	const pending = resolve
	resolve = null
	generation++
	busy.value = false
	pending?.(path)
	if (hide) modal.value?.hide()
}
function cancel() {
	settle(null)
}
function show(value: OptifineReference): Promise<string | null> {
	// One prompt at a time; never overwrite another import's pending promise.
	if (resolve) return Promise.resolve(null)
	reference.value = value
	modal.value?.show()
	return new Promise((done) => {
		resolve = done
	})
}
async function download() {
	try {
		await openOptifineDownloads()
	} catch (error) {
		handleError(error as Error)
	}
}
async function choose() {
	if (!reference.value) return
	const ticket = generation
	busy.value = true
	try {
		const selected = await pickOptifineInstaller(reference.value.minecraftVersion, reference.value)
		if (ticket === generation && selected) settle(selected.path)
	} catch (error) {
		if (ticket === generation) handleError(error as Error)
	} finally {
		if (ticket === generation) busy.value = false
	}
}
defineExpose({ show })
</script>
