<template>
	<NewModal
		ref="modal"
		:noblur="noblur"
		:danger="danger"
		:on-hide="onHide"
		max-width="800px"
		:header="title"
	>
		<div class="flex flex-col gap-4">
			<template v-if="description">
				<div
					v-if="markdown"
					class="markdown-body max-w-[35rem]"
					v-html="renderString(description)"
				/>
				<p v-else class="max-w-[35rem] m-0">
					{{ description }}
				</p>
			</template>
			<slot />
			<label v-if="hasToType" for="confirmation">
				<IntlFormatted :message-id="messages.typeToConfirm">
					<template #confirmation-text>
						<span class="font-semibold text-contrast">{{ confirmationText }}</span>
					</template>
				</IntlFormatted>
			</label>
			<Input
				v-if="hasToType"
				id="confirmation"
				v-model="confirmation_typed"
				:placeholder="formatMessage(messages.typeHerePlaceholder)"
				wrapper-class="max-w-[20rem]"
			/>
			<div class="flex gap-2 justify-end">
				<Button @click="hide()">
					<XIcon />
					{{ formatMessage(commonMessages.cancelButton) }}
				</Button>
				<Button
					type="colored"
					:color="danger ? 'red' : 'brand'"
					:disabled="action_disabled"
					@click="proceed"
				>
					<component :is="proceedIcon" />
					{{ proceedLabel }}
				</Button>
			</div>
		</div>
	</NewModal>
</template>

<script setup>
import { TrashIcon, XIcon } from '@orbiont/assets'
import { renderString } from '@orbiont/utils'
import { computed, ref } from 'vue'

import { Button } from '#ui/components/base/buttons'
import IntlFormatted from '#ui/components/base/IntlFormatted.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { commonMessages } from '#ui/utils/common-messages'

import Input from '../base/inputs/Input.vue'
import NewModal from './NewModal.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	typeToConfirm: {
		id: 'modal.confirm.type-to-confirm',
		defaultMessage:
			'To confirm you want to proceed, type <confirmation-text></confirmation-text> below:',
	},
	typeHerePlaceholder: {
		id: 'modal.confirm.type-here-placeholder',
		defaultMessage: 'Type here...',
	},
})

const props = defineProps({
	confirmationText: {
		type: String,
		default: '',
	},
	hasToType: {
		type: Boolean,
		default: false,
	},
	title: {
		type: String,
		default: 'No title defined',
		required: true,
	},
	description: {
		type: String,
		default: undefined,
		required: false,
	},
	proceedIcon: {
		type: Object,
		default: () => TrashIcon,
	},
	proceedLabel: {
		type: String,
		default: 'Proceed',
	},
	noblur: {
		type: Boolean,
		default: false,
	},
	danger: {
		type: Boolean,
		default: true,
	},
	onHide: {
		type: Function,
		default() {
			return () => {}
		},
	},
	markdown: {
		type: Boolean,
		default: true,
	},
})

const emit = defineEmits(['proceed'])
const modal = ref(null)

const confirmation_typed = ref('')

const action_disabled = computed(
	() =>
		props.hasToType &&
		confirmation_typed.value.toLowerCase() !== props.confirmationText.toLowerCase(),
)

function proceed() {
	modal.value.hide()
	confirmation_typed.value = ''
	emit('proceed')
}

function show() {
	modal.value.show()
}
function hide() {
	modal.value.hide()
}

defineExpose({ show, hide })
</script>
