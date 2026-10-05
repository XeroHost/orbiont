<template>
	<NewModal ref="modal" :header="formatMessage(messages.title)" @hide="settle(null, false)">
		<div class="flex max-w-[480px] flex-col gap-4 p-5">
			<p v-if="file" class="m-0 break-words text-secondary">
				{{ formatMessage(messages.description, { file: file.fileName }) }}
			</p>
			<div class="flex flex-wrap gap-2">
				<Button type="outlined" :disabled="!file?.pageUrl" @click="download">
					<DownloadIcon />{{ formatMessage(commonMessages.downloadButton) }}
				</Button>
				<Button type="colored" color="green" :disabled="busy" @click="choose">
					<UploadIcon />{{ formatMessage(messages.select) }}
				</Button>
				<Button type="outlined" @click="settle(null)">{{
					formatMessage(commonMessages.cancelButton)
				}}</Button>
			</div>
		</div>
	</NewModal>
</template>

<script setup lang="ts">
import { DownloadIcon, UploadIcon } from '@orbiont/assets'
import {
	Button,
	commonMessages,
	defineMessages,
	injectNotificationManager,
	NewModal,
	useVIntl,
} from '@orbiont/ui'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { openUrl } from '@tauri-apps/plugin-opener'
import { onUnmounted, ref, useTemplateRef } from 'vue'

import type { CurseforgeManualDownload } from '@/helpers/curseforge'

const messages = defineMessages({
	title: { id: 'curseforge.manual.title', defaultMessage: 'Manual download required' },
	description: {
		id: 'curseforge.manual.description',
		defaultMessage:
			'Download {file} from its official CurseForge page, then select the downloaded file to continue. Its size and SHA-1 will be verified.',
	},
	select: { id: 'curseforge.manual.select', defaultMessage: 'Select downloaded file' },
})
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')
const file = ref<CurseforgeManualDownload | null>(null)
const busy = ref(false)
let resolve: ((path: string | null) => void) | null = null
let generation = 0
function settle(path: string | null, hide = true) {
	const pending = resolve
	resolve = null
	generation++
	busy.value = false
	pending?.(path)
	if (hide) modal.value?.hide()
}
function show(value: CurseforgeManualDownload): Promise<string | null> {
	if (resolve) return Promise.resolve(null)
	file.value = value
	modal.value?.show()
	return new Promise((done) => {
		resolve = done
	})
}
async function download() {
	if (file.value?.pageUrl) await openUrl(file.value.pageUrl).catch(handleError)
}
async function choose() {
	if (!file.value || busy.value) return
	const selectedFile = file.value
	const ticket = generation
	busy.value = true
	try {
		const extension = selectedFile.fileName.split('.').pop() ?? 'jar'
		const path = await open({
			multiple: false,
			title: selectedFile.fileName,
			filters: [{ name: 'CurseForge', extensions: [extension] }],
		})
		if (ticket !== generation || typeof path !== 'string') return
		const verifiedPath = await invoke<string>('plugin:orbiont|orbiont_import_curseforge_file', {
			modId: selectedFile.projectId,
			fileId: selectedFile.fileId,
			path,
		})
		if (ticket === generation) settle(verifiedPath)
	} catch (error) {
		if (ticket === generation) handleError(error as Error)
	} finally {
		if (ticket === generation) busy.value = false
	}
}
onUnmounted(() => settle(null, false))
defineExpose({ show })
</script>
