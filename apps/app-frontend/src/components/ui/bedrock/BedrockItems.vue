<script setup lang="ts">
import { FolderOpenIcon, SearchIcon, TrashIcon } from '@orbiont/assets'
import {
	Button,
	commonMessages,
	ContentCardTable,
	type ContentCardTableItem,
	EmptyState,
	Input,
	useVIntl,
} from '@orbiont/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { computed, ref, watch } from 'vue'

import type { BedrockItem } from '@/helpers/bedrock'
import { bedrockMessages as messages } from '@/helpers/bedrock-messages'

const props = defineProps<{ items: BedrockItem[]; emptyMessage: string; busy: boolean }>()
const emit = defineEmits<{
	openFolder: [rootId: string, path: string]
	delete: [items: BedrockItem[]]
}>()
const { formatMessage } = useVIntl()
const search = ref('')
const selectedIds = ref<string[]>([])
const sortDirection = ref<'asc' | 'desc'>('asc')
watch(
	() => props.items,
	(items) => {
		selectedIds.value = selectedIds.value.filter((id) =>
			items.some((item) => `${item.root_id}/${item.path}` === id),
		)
	},
)
const filtered = computed<ContentCardTableItem[]>(() => {
	const needle = search.value.trim().toLocaleLowerCase()
	return props.items
		.filter((item) =>
			`${item.name} ${item.description ?? ''} ${item.path}`.toLocaleLowerCase().includes(needle),
		)
		.toSorted((a, b) => a.name.localeCompare(b.name) * (sortDirection.value === 'asc' ? 1 : -1))
		.map((item) => {
			const id = `${item.root_id}/${item.path}`
			return {
				id,
				project: {
					id,
					slug: id,
					title: item.name,
					icon_url: item.icon_path ? convertFileSrc(item.icon_path) : '',
				},
				subtitle: item.path.split('/').at(-1),
				hideToggle: true,
				hideDelete: false,
				disabled: props.busy,
				hideSwitchVersion: true,
				overflowOptions: [
					{
						id: 'open-folder',
						label: formatMessage(messages.openFolder),
						icon: FolderOpenIcon,
						disabled: props.busy,
						action: () => emit('openFolder', item.root_id, item.path),
					},
				],
			}
		})
})
function deleteById(id: string) {
	if (props.busy) return
	const item = props.items.find((item) => `${item.root_id}/${item.path}` === id)
	if (item) emit('delete', [item])
}
function deleteSelected() {
	if (props.busy) return
	emit(
		'delete',
		props.items.filter((item) => selectedIds.value.includes(`${item.root_id}/${item.path}`)),
	)
}
</script>

<template>
	<div class="flex min-w-0 flex-col gap-4">
		<div class="flex flex-wrap items-center gap-3">
			<Input
				v-model="search"
				:icon="SearchIcon"
				:placeholder="formatMessage(messages.search)"
				:aria-label="formatMessage(messages.search)"
				clearable
				class="min-w-0 flex-1"
			/>
			<Button
				v-if="selectedIds.length"
				type="outlined"
				color="red"
				:disabled="busy"
				@click="deleteSelected"
				><TrashIcon />{{ formatMessage(commonMessages.deleteLabel) }}
				<span class="tabular-nums">({{ selectedIds.length }})</span></Button
			>
		</div>
		<EmptyState v-if="!items.length" type="empty-inbox"
			><template #heading>{{ emptyMessage }}</template></EmptyState
		>
		<ContentCardTable
			v-else
			v-model:selected-ids="selectedIds"
			:items="filtered"
			show-selection
			sortable
			sort-by="project"
			:sort-direction="sortDirection"
			show-item-actions
			:show-version="false"
			@sort="(_, direction) => (sortDirection = direction)"
			@delete="deleteById"
		>
			<template #empty>{{ formatMessage(messages.noResults) }}</template>
		</ContentCardTable>
	</div>
</template>
