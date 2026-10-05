<script setup lang="ts">
import { FolderOpenIcon } from '@orbiont/assets'
import { Admonition, Button, Combobox, useVIntl } from '@orbiont/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'

import { type BedrockItem, type BedrockRoot, readBedrockLog } from '@/helpers/bedrock'
import { bedrockMessages as messages } from '@/helpers/bedrock-messages'

const props = defineProps<{ items: BedrockItem[]; roots: BedrockRoot[]; busy: boolean }>()
const emit = defineEmits<{ openFolder: [rootId: string, path: string] }>()
const { formatMessage } = useVIntl()
const selection = ref('')
const key = (item: BedrockItem) => `${item.root_id}/${item.path}`
const logs = computed(() => props.items.filter((item) => item.kind === 'log'))
watch(
	logs,
	(available) => {
		if (!available.some((item) => key(item) === selection.value))
			selection.value = available[0] ? key(available[0]) : ''
	},
	{ immediate: true },
)
const selected = computed(() => logs.value.find((item) => key(item) === selection.value))
const options = computed(() =>
	logs.value.map((item) => ({
		value: key(item),
		label: item.name,
		subLabel: props.roots.find((root) => root.id === item.root_id)?.path,
	})),
)
const log = useQuery(
	computed(() => ({
		queryKey: ['bedrock', 'log', selection.value],
		queryFn: () => readBedrockLog(selected.value!.root_id, selected.value!.path),
		enabled: !!selected.value,
		retry: false,
	})),
)
</script>

<template>
	<div class="flex flex-col gap-4">
		<p v-if="!logs.length" class="rounded-xl bg-surface-2 p-6 text-secondary">
			{{ formatMessage(messages.noLogs) }}
		</p>
		<template v-else>
			<Combobox
				v-model="selection"
				:options="options"
				:aria-label="formatMessage(messages.selectLog)"
			/>
			<div class="flex flex-wrap gap-3">
				<Button
					type="outlined"
					:disabled="busy || !selected"
					@click="selected && emit('openFolder', selected.root_id, '')"
					><FolderOpenIcon />{{ formatMessage(messages.openFolder) }}</Button
				>
				<Button type="outlined" :disabled="log.isFetching.value" @click="log.refetch()">{{
					formatMessage(messages.refreshData)
				}}</Button>
			</div>
			<p v-if="log.isPending.value" role="status">{{ formatMessage(messages.loadingData) }}</p>
			<Admonition v-else-if="log.isError.value" type="warning" role="alert">{{
				formatMessage(messages.dataError)
			}}</Admonition>
			<template v-else>
				<Admonition v-if="log.data.value?.truncated" type="info">{{
					formatMessage(messages.logTruncated)
				}}</Admonition>
				<pre
					class="m-0 max-h-[60vh] overflow-auto whitespace-pre-wrap break-all rounded-xl bg-surface-2 p-4 text-sm text-primary"
					>{{ log.data.value?.text }}</pre
				>
			</template>
		</template>
	</div>
</template>
