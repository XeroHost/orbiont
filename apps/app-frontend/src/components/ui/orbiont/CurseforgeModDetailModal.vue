<script setup lang="ts">
import { DownloadIcon, ExternalIcon, UploadIcon } from '@modrinth/assets'
import { Avatar, Button, NewModal, useFileDropTarget } from '@modrinth/ui'
import { renderString } from '@modrinth/utils/parse'
import { useQuery } from '@tanstack/vue-query'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, ref } from 'vue'

import type { SearchResult } from '@/helpers/orbiont'
import { downloadSearchResult, getCurseforgeModDetail, saveDroppedFile } from '@/helpers/orbiont'
import { injectAppEvents } from '@/providers/app-events'
import { installSearchResultFile } from '@/providers/orbiont-play'

const emit = defineEmits<{ error: [err: unknown] }>()

const modal = ref<InstanceType<typeof NewModal>>()
const currentResult = ref<SearchResult | null>(null)
const appEvents = injectAppEvents()
const installing = ref(false)
const showDropZone = ref(false)
const dropTarget = ref<HTMLElement | null>(null)

const modId = computed(() => currentResult.value?.id.split(':')[1] ?? null)

const detailQuery = useQuery({
	queryKey: computed(() => ['orbiont', 'curseforge-mod-detail', modId.value]),
	queryFn: () => getCurseforgeModDetail(modId.value as string),
	enabled: () => !!modId.value,
})

function show(result: SearchResult) {
	currentResult.value = result
	showDropZone.value = false
	modal.value?.show()
}

defineExpose({ show })

const { isDragging, dropTargetProps } = useFileDropTarget({
	target: dropTarget,
	disabled: installing,
	onFiles: async (files) => {
		const file = files[0]
		if (!file || !currentResult.value) return
		installing.value = true
		try {
			const bytes = new Uint8Array(await file.arrayBuffer())
			const path = await saveDroppedFile(bytes, `${currentResult.value.id}-${file.name}`)
			await installSearchResultFile(currentResult.value, path, appEvents)
			modal.value?.hide()
		} catch (err) {
			emit('error', err)
		} finally {
			installing.value = false
		}
	},
	onError: (err) => emit('error', err),
})

async function installFromUrl() {
	if (!currentResult.value?.downloadUrl || installing.value) return
	installing.value = true
	try {
		const path = await downloadSearchResult(
			currentResult.value.downloadUrl,
			`${currentResult.value.id}.mrpack`,
		)
		await installSearchResultFile(currentResult.value, path, appEvents)
		modal.value?.hide()
	} catch (err) {
		emit('error', err)
	} finally {
		installing.value = false
	}
}

async function openPage() {
	if (currentResult.value?.pageUrl) {
		await openUrl(currentResult.value.pageUrl)
	}
}
</script>

<template>
	<NewModal ref="modal" scrollable :header="currentResult?.name" max-width="42rem" actions-divider>
		<div v-if="detailQuery.isLoading.value" class="p-2 text-sm text-secondary">Loading…</div>
		<div v-else-if="detailQuery.isError.value" class="p-2 text-sm text-red">
			{{ (detailQuery.error.value as Error)?.message ?? 'Failed to load details.' }}
		</div>
		<div v-else-if="detailQuery.data.value" class="flex flex-col gap-4">
			<div class="flex items-center gap-3">
				<Avatar :src="detailQuery.data.value.icon ?? undefined" size="64px" class="rounded-xl" />
				<div class="flex min-w-0 flex-1 flex-col gap-0.5">
					<span class="truncate text-sm text-secondary">{{ detailQuery.data.value.summary }}</span>
					<span v-if="detailQuery.data.value.authors.length > 0" class="text-sm text-secondary">
						by {{ detailQuery.data.value.authors.join(', ') }}
					</span>
				</div>
			</div>

			<div
				v-if="detailQuery.data.value.screenshots.length > 0"
				class="flex gap-2 overflow-x-auto pb-1"
			>
				<img
					v-for="shot in detailQuery.data.value.screenshots"
					:key="shot.url"
					:src="shot.thumbnailUrl"
					:alt="shot.title"
					class="h-32 w-auto shrink-0 rounded-xl object-cover"
				/>
			</div>

			<div class="flex flex-wrap gap-2">
				<span
					v-for="category in detailQuery.data.value.categories"
					:key="category"
					class="rounded-full bg-button-bg px-2 py-0.5 text-xs font-medium text-secondary"
				>
					{{ category }}
				</span>
			</div>

			<div
				class="markdown-body text-sm text-primary"
				v-html="renderString(detailQuery.data.value.description)"
			></div>

			<div
				v-if="showDropZone"
				ref="dropTarget"
				v-bind="dropTargetProps"
				class="flex items-center justify-center rounded-xl border-2 border-dashed p-4 text-sm text-secondary"
				:class="isDragging ? 'border-brand bg-brand-highlight' : 'border-button-border'"
			>
				{{ installing ? 'Installing…' : 'Drop the downloaded modpack file here' }}
			</div>
		</div>

		<template #actions>
			<div class="flex justify-end gap-2">
				<Button v-if="currentResult?.downloadUrl" :loading="installing" @click="installFromUrl">
					<DownloadIcon />
					Install
				</Button>
				<template v-else>
					<Button v-if="currentResult?.pageUrl" type="outlined" @click="openPage">
						<ExternalIcon />
						Open mod page
					</Button>
					<Button :disabled="installing" @click="showDropZone = !showDropZone">
						<UploadIcon />
						I have the file
					</Button>
				</template>
			</div>
		</template>
	</NewModal>
</template>
