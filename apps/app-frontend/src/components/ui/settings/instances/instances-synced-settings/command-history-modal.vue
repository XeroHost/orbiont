<script setup lang="ts">
import { SaveIcon, XIcon } from '@orbiont/assets'
import {
	Button,
	commonMessages,
	defineMessages,
	injectNotificationManager,
	NewModal,
	useVIntl,
} from '@orbiont/ui'
import { loadAceEditor } from '@orbiont/ui/src/utils/ace-loader'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import type { Component } from 'vue'
import { nextTick, ref, shallowRef } from 'vue'

import { set_command_history } from '@/helpers/instance'
import { commandHistoryQueryOptions, syncedOptionsKeys } from '@/helpers/synced-options'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const queryClient = useQueryClient()
const modal = ref<InstanceType<typeof NewModal> | null>(null)
const commandHistory = ref('')
const historyQuery = useQuery({ ...commandHistoryQueryOptions(), enabled: false })
const commandHistoryEditor = shallowRef<Component | null>(null)
const isOpening = ref(false)
const saveMutation = useMutation({
	mutationFn: set_command_history,
	onSuccess: (history) => {
		queryClient.setQueryData(syncedOptionsKeys.commandHistory, history)
	},
	onError: handleError,
})

const messages = defineMessages({
	commandHistoryEditorTitle: {
		id: 'app.settings.synced-options.command-history.editor-title',
		defaultMessage: 'Edit command history',
	},
})

async function show() {
	if (isOpening.value) return
	isOpening.value = true
	try {
		const result = await historyQuery.refetch()
		if (result.isError) {
			handleError(result.error)
			return
		}
		if (!result.isSuccess) return
		commandHistoryEditor.value = await loadAceEditor('mcfunction')
		commandHistory.value = result.data
		modal.value?.show()
	} catch (error) {
		handleError(error)
	} finally {
		isOpening.value = false
	}
}

async function saveCommandHistory() {
	if (saveMutation.isPending.value) return
	try {
		await saveMutation.mutateAsync(commandHistory.value)
		await nextTick()
		modal.value?.hide()
	} catch {
		return
	}
}

defineExpose({ show })
</script>

<template>
	<NewModal
		ref="modal"
		:header="formatMessage(messages.commandHistoryEditorTitle)"
		:disable-close="saveMutation.isPending.value"
		no-padding
		actions-divider
		max-width="700px"
		width="700px"
	>
		<component
			:is="commandHistoryEditor"
			v-if="commandHistoryEditor"
			v-model:value="commandHistory"
			lang="mcfunction"
			theme="modrinth"
			:print-margin="false"
			class="command-history-editor ace-modrinth"
			style="height: 420px; font-size: 0.875rem"
		/>
		<template #actions>
			<div class="flex justify-end gap-2">
				<Button type="outlined" @click="modal?.hide()">
					<XIcon aria-hidden="true" />
					{{ formatMessage(commonMessages.cancelButton) }}
				</Button>
				<Button type="colored" :loading="saveMutation.isPending.value" @click="saveCommandHistory">
					<SaveIcon aria-hidden="true" />
					{{ formatMessage(commonMessages.saveButton) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>

<style>
.command-history-editor.ace-modrinth {
	background-color: var(--surface-2);
}

.command-history-editor.ace-modrinth .ace_gutter {
	background: var(--surface-1);
}

.command-history-editor.ace-modrinth .ace_marker-layer .ace_active-line {
	background: var(--surface-2-5);
}

.command-history-editor.ace-modrinth .ace_gutter-active-line {
	background-color: var(--surface-1-5);
}

.command-history-editor.ace-modrinth.ace_multiselect .ace_selection.ace_start {
	box-shadow: 0 0 3px 0 var(--surface-2);
}
</style>
