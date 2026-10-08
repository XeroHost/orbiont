<script setup lang="ts">
import { TrashIcon } from '@orbiont/assets'
import { Admonition, Button, commonMessages, NewModal, useVIntl } from '@orbiont/ui'
import { onUnmounted, ref } from 'vue'

import type { BedrockItem } from '@/helpers/bedrock'
import { bedrockMessages as messages } from '@/helpers/bedrock-messages'

const props = defineProps<{ running: boolean }>()
const { formatMessage } = useVIntl()
const deletion = ref<InstanceType<typeof NewModal>>()
const pending = ref<BedrockItem[]>([])
let resolvePrompt: ((result: boolean) => void) | undefined
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
onUnmounted(() => resolvePrompt?.(false))
defineExpose({ confirm })
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
</template>
