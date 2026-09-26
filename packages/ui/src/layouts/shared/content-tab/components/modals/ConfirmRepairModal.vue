<template>
	<NewModal
		ref="modal"
		:header="
			formatMessage(messages.header, {
				type: formatMessage(messages.instanceLabel),
			})
		"
		max-width="500px"
	>
		<span class="text-primary">
			{{ formatMessage(messages.instanceBody) }}
		</span>

		<template #actions>
			<div class="flex gap-2 justify-end">
				<Button type="outlined" @click="modal?.hide()">
					<XIcon />
					{{ formatMessage(commonMessages.cancelButton) }}
				</Button>
				<Button type="colored" color="green" @click="confirm">
					<HammerIcon />
					{{ formatMessage(messages.repairButton) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>

<script setup lang="ts">
import { HammerIcon, XIcon } from '@modrinth/assets'
import { ref } from 'vue'

import { Button } from '#ui/components/base/buttons'
import NewModal from '#ui/components/modal/NewModal.vue'
import { useDebugLogger } from '#ui/composables/debug-logger'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { commonMessages } from '#ui/utils/common-messages'

const { formatMessage } = useVIntl()
const debug = useDebugLogger('ConfirmRepairModal')

const messages = defineMessages({
	header: {
		id: 'instance.confirm-repair.header',
		defaultMessage: 'Repair {type}',
	},
	instanceBody: {
		id: 'instance.confirm-repair.body.instance',
		defaultMessage:
			'Repairing reinstalls the loader and Minecraft dependencies without deleting your content. This may resolve issues if your game is not launching due to launcher-related errors.',
	},
	repairButton: {
		id: 'instance.confirm-repair.repair-button',
		defaultMessage: 'Repair',
	},
	instanceLabel: {
		id: 'instance.confirm-repair.instance-label',
		defaultMessage: 'instance',
	},
})

const emit = defineEmits<{
	(e: 'repair'): void
}>()

const modal = ref<InstanceType<typeof NewModal>>()

function show() {
	debug('show: called', { hasModalRef: !!modal.value })
	modal.value?.show()
	debug('show: returned from modal.show', { hasModalRef: !!modal.value })
}

function confirm() {
	debug('confirm: called', { hasModalRef: !!modal.value })
	modal.value?.hide()
	emit('repair')
	debug('confirm: emitted repair')
}

defineExpose({
	show,
})
</script>
