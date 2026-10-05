<script setup lang="ts">
import { HistoryIcon, TrashIcon } from '@orbiont/assets'
import { Admonition, Button, commonMessages, NewModal, useVIntl } from '@orbiont/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { onUnmounted, ref } from 'vue'

import { type BedrockItem, listBedrockRecoveries, restoreBedrockItem } from '@/helpers/bedrock'
import { bedrockMessages as messages } from '@/helpers/bedrock-messages'

const props = defineProps<{ running: boolean }>()
const { formatMessage, locale } = useVIntl()
const formatDateTime = (date: Date) =>
	new Intl.DateTimeFormat(locale.value, { dateStyle: 'short', timeStyle: 'short' }).format(date)
const queryClient = useQueryClient()
const deletion = ref<InstanceType<typeof NewModal>>()
const recovery = ref<InstanceType<typeof NewModal>>()
const pending = ref<BedrockItem[]>([])
const restoring = ref(false)
const restoreError = ref('')
const opened = ref(false)
let resolvePrompt: ((result: boolean) => void) | undefined
const copies = useQuery({
	queryKey: ['bedrock', 'recoveries'],
	queryFn: listBedrockRecoveries,
	enabled: opened,
	retry: false,
})
function settle(result: boolean) {
	const resolve = resolvePrompt
	resolvePrompt = undefined
	resolve?.(result)
}
function finish(result: boolean) {
	settle(result)
	deletion.value?.hide()
}
function confirm(items: BedrockItem[]): Promise<boolean> {
	if (props.running || !items.length || resolvePrompt) return Promise.resolve(false)
	pending.value = items
	return new Promise((resolve) => {
		resolvePrompt = resolve
		deletion.value?.show()
	})
}
async function showRecovery() {
	opened.value = true
	restoreError.value = ''
	recovery.value?.show()
	await copies.refetch()
}
async function restore(rootId: string, id: string) {
	if (restoring.value || props.running) return
	restoring.value = true
	restoreError.value = ''
	try {
		await restoreBedrockItem(rootId, id)
		await queryClient.invalidateQueries({ queryKey: ['bedrock'] })
	} catch (error) {
		restoreError.value = formatMessage(
			(error as { code?: string })?.code === 'conflict'
				? messages.conflictError
				: messages.mutationError,
		)
	} finally {
		restoring.value = false
	}
}
onUnmounted(() => resolvePrompt?.(false))
defineExpose({ confirm, showRecovery })
</script>

<template>
	<NewModal
		ref="deletion"
		:header="formatMessage(messages.deleteTitle)"
		fade="warning"
		max-width="560px"
		:on-hide="() => settle(false)"
	>
		<div class="flex flex-col gap-4">
			<Admonition type="warning">{{ formatMessage(messages.deleteHelp) }}</Admonition>
			<ul class="m-0 max-h-48 overflow-auto pl-5">
				<li v-for="item in pending" :key="`${item.root_id}/${item.path}`" class="break-words">
					{{ item.name }}
				</li>
			</ul>
		</div>
		<template #actions
			><div class="flex justify-end gap-2">
				<Button type="outlined" @click="finish(false)">{{
					formatMessage(commonMessages.cancelButton)
				}}</Button>
				<Button type="colored" color="red" :disabled="running" @click="finish(true)"
					><TrashIcon />{{ formatMessage(commonMessages.deleteLabel) }}</Button
				>
			</div></template
		>
	</NewModal>
	<NewModal
		ref="recovery"
		:header="formatMessage(messages.recoveryTitle)"
		max-width="720px"
		:disable-close="restoring"
	>
		<div class="flex flex-col gap-4">
			<p class="m-0 text-secondary">{{ formatMessage(messages.recoveryHelp) }}</p>
			<Admonition v-if="running" type="info">{{
				formatMessage(messages.closeToManage)
			}}</Admonition>
			<Admonition v-if="restoreError || copies.isError.value" type="warning" role="alert">{{
				restoreError || formatMessage(messages.dataError)
			}}</Admonition>
			<p v-if="copies.isPending.value" role="status">{{ formatMessage(messages.loadingData) }}</p>
			<p v-else-if="!copies.data.value?.length && !copies.isError.value" class="text-secondary">
				{{ formatMessage(messages.noRecoveries) }}
			</p>
			<div
				v-for="copy in copies.data.value ?? []"
				:key="copy.id"
				class="flex items-center justify-between gap-4 rounded-xl bg-surface-3 p-4"
			>
				<div class="min-w-0">
					<p class="m-0 break-all font-semibold">{{ copy.path }}</p>
					<p class="mb-0 mt-1 text-sm text-secondary">
						{{ formatDateTime(new Date(copy.saved_at * 1000)) }}
					</p>
				</div>
				<Button
					type="outlined"
					:disabled="running || restoring"
					@click="restore(copy.root_id, copy.id)"
					><HistoryIcon />{{ formatMessage(messages.restore) }}</Button
				>
			</div>
		</div>
	</NewModal>
</template>
