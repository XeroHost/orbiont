<script setup lang="ts">
import { Button, defineMessages, injectNotificationManager, useVIntl } from '@orbiont/ui'
import { Channel, invoke } from '@tauri-apps/api/core'
import { onBeforeUnmount, ref } from 'vue'

import ConfirmModalWrapper from '@/components/ui/modal/ConfirmModalWrapper.vue'
import { createDiagnosticExport } from '@/helpers/diagnostic-export'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const confirmation = ref<InstanceType<typeof ConfirmModalWrapper>>()
const state = ref({ busy: false, completed: 0, total: 0, saved: false })
const messages = defineMessages({
	title: { id: 'app.settings.diagnostics.title', defaultMessage: 'Diagnostic archive' },
	description: {
		id: 'app.settings.diagnostics.description',
		defaultMessage:
			'Save app and operating system versions and up to eight sanitized launcher logs to a local ZIP file. Tokens, custom environment values and user paths are redacted. No database, account records, game files or uploads are included. Review the archive before sharing it.',
	},
	export: { id: 'app.settings.diagnostics.export', defaultMessage: 'Save diagnostic archive' },
	cancel: { id: 'app.settings.diagnostics.cancel', defaultMessage: 'Cancel export' },
	progress: {
		id: 'app.settings.diagnostics.progress',
		defaultMessage: 'Exporting logs: {completed} of {total}',
	},
	saved: {
		id: 'app.settings.diagnostics.saved',
		defaultMessage: 'Diagnostic archive saved locally.',
	},
})
const exporter = createDiagnosticExport({
	async save(operation, onProgress) {
		const progress = new Channel<{ completed: number; total: number }>()
		progress.onmessage = onProgress
		return await invoke<boolean>('plugin:utils|export_debug_info', {
			operation,
			progress,
			runtimeVersion: navigator.userAgent,
		})
	},
	cancel: (operation) => invoke('plugin:utils|cancel_debug_info', { operation }),
	changed: (value) => {
		state.value = value
	},
})
function start() {
	void exporter.start().catch(handleError)
}
function cancel() {
	void exporter.cancel().catch(handleError)
}
onBeforeUnmount(() => {
	void exporter.cancel().catch(handleError)
})
</script>

<template>
	<div class="flex flex-col gap-2.5">
		<ConfirmModalWrapper
			ref="confirmation"
			:title="formatMessage(messages.title)"
			:description="formatMessage(messages.description)"
			:proceed-label="formatMessage(messages.export)"
			:danger="false"
			:show-ad-on-close="false"
			@proceed="start"
		/>
		<h2 class="m-0 text-lg font-semibold text-contrast">{{ formatMessage(messages.title) }}</h2>
		<p class="m-0 leading-tight text-secondary">{{ formatMessage(messages.description) }}</p>
		<Button class="w-fit" :disabled="state.busy" @click="confirmation?.show()">{{
			formatMessage(messages.export)
		}}</Button>
		<template v-if="state.busy">
			<p role="status">
				{{ formatMessage(messages.progress, { completed: state.completed, total: state.total }) }}
			</p>
			<progress :value="state.completed" :max="Math.max(1, state.total)" />
			<Button class="w-fit" @click="cancel">{{ formatMessage(messages.cancel) }}</Button>
		</template>
		<p v-else-if="state.saved" role="status">{{ formatMessage(messages.saved) }}</p>
	</div>
</template>
