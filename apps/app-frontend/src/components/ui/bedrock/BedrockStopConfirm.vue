<script setup lang="ts">
import { StopCircleIcon } from '@orbiont/assets'
import { ConfirmModal, useVIntl } from '@orbiont/ui'
import { onUnmounted, ref } from 'vue'

import { bedrockMessages as messages } from '@/helpers/bedrock-messages'

const { formatMessage } = useVIntl()
const modal = ref<InstanceType<typeof ConfirmModal>>()
let resolveAnswer: ((answer: boolean) => void) | undefined
function settle(answer: boolean) {
	const resolve = resolveAnswer
	resolveAnswer = undefined
	resolve?.(answer)
}
function ask(): Promise<boolean> {
	if (resolveAnswer || !modal.value) return Promise.resolve(false)
	return new Promise((resolve) => {
		resolveAnswer = resolve
		modal.value?.show()
	})
}
function onHide() {
	queueMicrotask(() => settle(false))
}
onUnmounted(() => settle(false))
defineExpose({ ask })
</script>

<template>
	<ConfirmModal
		ref="modal"
		:title="formatMessage(messages.closeTimeout)"
		:description="formatMessage(messages.forceStopHelp)"
		:proceed-label="formatMessage(messages.forceStop)"
		:proceed-icon="StopCircleIcon"
		:danger="true"
		:markdown="false"
		:on-hide="onHide"
		@proceed="settle(true)"
	/>
</template>
