<script setup lang="ts">
import { DownloadIcon, ExternalIcon, UploadIcon } from '@modrinth/assets'
import { Avatar, Button, useFileDropTarget } from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { ref } from 'vue'

import CurseforgeModDetailModal from '@/components/ui/orbiont/CurseforgeModDetailModal.vue'
import type { SearchResult } from '@/helpers/orbiont'
import { downloadSearchResult, saveDroppedFile } from '@/helpers/orbiont'
import { injectAppEvents } from '@/providers/app-events'
import { installSearchResultFile } from '@/providers/orbiont-play'

const props = defineProps<{ result: SearchResult }>()
const emit = defineEmits<{ error: [err: unknown] }>()

const appEvents = injectAppEvents()
const installing = ref(false)
const showDropZone = ref(false)
const dropTarget = ref<HTMLElement | null>(null)
const detailModal = ref<InstanceType<typeof CurseforgeModDetailModal>>()

const { isDragging, dropTargetProps } = useFileDropTarget({
	target: dropTarget,
	disabled: installing,
	onFiles: async (files) => {
		const file = files[0]
		if (!file) return
		installing.value = true
		try {
			const bytes = new Uint8Array(await file.arrayBuffer())
			const path = await saveDroppedFile(bytes, `${props.result.id}-${file.name}`)
			await installSearchResultFile(props.result, path, appEvents)
			showDropZone.value = false
		} catch (err) {
			emit('error', err)
		} finally {
			installing.value = false
		}
	},
	onError: (err) => emit('error', err),
})

async function installFromUrl() {
	if (!props.result.downloadUrl || installing.value) return
	installing.value = true
	try {
		const path = await downloadSearchResult(props.result.downloadUrl, `${props.result.id}.mrpack`)
		await installSearchResultFile(props.result, path, appEvents)
	} catch (err) {
		emit('error', err)
	} finally {
		installing.value = false
	}
}

async function openPage() {
	if (props.result.pageUrl) {
		await openUrl(props.result.pageUrl)
	}
}

function openDetail() {
	if (props.result.source === 'curseforge') {
		detailModal.value?.show(props.result)
	}
}
</script>

<template>
	<div class="flex flex-col gap-2 rounded-2xl bg-bg-raised p-3">
		<div class="flex items-center gap-3">
			<button
				type="button"
				class="flex min-w-0 flex-1 items-center gap-3 border-none bg-transparent p-0 text-left"
				:class="result.source === 'curseforge' ? 'cursor-pointer' : 'cursor-default'"
				@click="openDetail"
			>
				<Avatar :src="result.icon ?? undefined" size="48px" class="rounded-xl" />
				<div class="flex min-w-0 flex-1 flex-col gap-0.5">
					<span
						class="truncate font-bold text-contrast"
						:class="{ 'hover:underline': result.source === 'curseforge' }"
					>
						{{ result.name }}
					</span>
					<span class="truncate text-sm text-secondary">{{ result.description }}</span>
				</div>
			</button>
			<Button v-if="result.downloadUrl" :loading="installing" @click="installFromUrl">
				<DownloadIcon />
				Install
			</Button>
			<template v-else>
				<Button v-if="result.pageUrl" type="outlined" @click="openPage">
					<ExternalIcon />
					Open mod page
				</Button>
				<Button :disabled="installing" @click="showDropZone = !showDropZone">
					<UploadIcon />
					I have the file
				</Button>
			</template>
		</div>

		<div
			v-if="showDropZone"
			ref="dropTarget"
			v-bind="dropTargetProps"
			class="flex items-center justify-center rounded-xl border-2 border-dashed p-4 text-sm text-secondary"
			:class="isDragging ? 'border-brand bg-brand-highlight' : 'border-button-border'"
		>
			{{ installing ? 'Installing…' : 'Drop the downloaded modpack file here' }}
		</div>

		<CurseforgeModDetailModal ref="detailModal" @error="(err) => emit('error', err)" />
	</div>
</template>
