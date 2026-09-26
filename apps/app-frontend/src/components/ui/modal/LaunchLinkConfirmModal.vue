<script setup lang="ts">
import { PlayIcon } from '@orbiont/assets'
import { ConfirmModal, defineMessages, useVIntl } from '@orbiont/ui'
import { computed, ref, useTemplateRef } from 'vue'

/**
 * Asks before an `orbiont://launch/...` link starts the game. Any website can
 * open such a link, so it never launches or joins a server on its own.
 */
const { formatMessage } = useVIntl()
const modal = useTemplateRef('modal')

type LaunchTarget = { kind: 'server' | 'world'; name: string } | null

const request = ref<{ instanceName: string; target: LaunchTarget } | null>(null)
let resolveAnswer: ((answer: boolean) => void) | null = null

function settle(answer: boolean) {
	resolveAnswer?.(answer)
	resolveAnswer = null
}

/** Resolves true only if the user confirms. */
function ask(instanceName: string, target: LaunchTarget): Promise<boolean> {
	settle(false)
	request.value = { instanceName, target }
	modal.value?.show()
	return new Promise((resolve) => {
		resolveAnswer = resolve
	})
}

// ConfirmModal hides itself before emitting "proceed": wait a tick so a
// confirmation isn't taken as a dismissal.
function onHide() {
	queueMicrotask(() => settle(false))
}

defineExpose({ ask })

const messages = defineMessages({
	title: { id: 'app.launch-link.title', defaultMessage: 'Open Minecraft from a link?' },
	server: {
		id: 'app.launch-link.server',
		defaultMessage:
			'A link wants to start {instance} and join the server {target}. Only continue if you trust where this link came from.',
	},
	world: {
		id: 'app.launch-link.world',
		defaultMessage: 'A link wants to start {instance} and open the world {target}.',
	},
	instance: {
		id: 'app.launch-link.instance',
		defaultMessage: 'A link wants to start {instance}.',
	},
	proceed: { id: 'app.launch-link.proceed', defaultMessage: 'Start' },
})

const description = computed(() => {
	const current = request.value
	if (!current) return ''
	const values = { instance: current.instanceName, target: current.target?.name ?? '' }
	if (!current.target) return formatMessage(messages.instance, values)
	return formatMessage(current.target.kind === 'server' ? messages.server : messages.world, values)
})
</script>

<template>
	<ConfirmModal
		ref="modal"
		:title="formatMessage(messages.title)"
		:description="description"
		:proceed-label="formatMessage(messages.proceed)"
		:proceed-icon="PlayIcon"
		:danger="false"
		:markdown="false"
		:on-hide="onHide"
		@proceed="settle(true)"
	/>
</template>
