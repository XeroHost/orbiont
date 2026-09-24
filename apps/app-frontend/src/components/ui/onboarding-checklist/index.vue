<script setup lang="ts">
import { CheckIcon, DropdownIcon, RadioButtonIcon } from '@modrinth/assets'
import { defineMessages, FloatingMenu, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import { injectOnboardingChecklist } from '@/providers/onboarding-checklist'

const emit = defineEmits<{
	'create-instance': []
	'login-minecraft': []
}>()

const { formatMessage } = useVIntl()
const { hasCreatedInstance, hasLoggedIntoMinecraft, isReady, showChecklist } =
	injectOnboardingChecklist()

const messages = defineMessages({
	title: {
		id: 'onboarding-checklist.title',
		defaultMessage: 'Getting started',
	},
	createInstance: {
		id: 'onboarding-checklist.create-instance',
		defaultMessage: 'Create first instance',
	},
	loginMinecraft: {
		id: 'onboarding-checklist.login-minecraft',
		defaultMessage: 'Sign in to Minecraft',
	},
})

const steps = computed(() => [
	{
		id: 'create-instance',
		label: formatMessage(messages.createInstance),
		complete: hasCreatedInstance.value,
		action: () => emit('create-instance'),
	},
	{
		id: 'login-minecraft',
		label: formatMessage(messages.loginMinecraft),
		complete: hasLoggedIntoMinecraft.value,
		action: () => emit('login-minecraft'),
	},
])
</script>

<template>
	<FloatingMenu v-if="isReady && showChecklist" placement="bottom-end">
		<button
			type="button"
			class="flex items-center gap-2 rounded-xl border border-solid border-surface-5 bg-button-bg px-3 py-1.5 text-sm font-medium text-contrast transition-[filter] hover:brightness-110"
		>
			{{ formatMessage(messages.title) }}
			<DropdownIcon class="size-4" />
		</button>
		<template #popper="{ hide }">
			<div class="flex w-64 flex-col gap-2 p-1">
				<button
					v-for="step in steps"
					:key="step.id"
					type="button"
					class="flex h-10 w-full items-center gap-2 rounded-xl border border-solid border-button-border bg-button-bg px-4 py-2.5 text-left text-primary shadow-[0_1px_0.5px_rgb(0_0_0_/_12%)] transition-[filter]"
					:class="
						step.complete
							? '!cursor-default opacity-50'
							: 'cursor-pointer hover:brightness-110 active:brightness-90'
					"
					:disabled="step.complete"
					@click="
						() => {
							step.action()
							hide()
						}
					"
				>
					<span
						v-if="step.complete"
						class="flex size-[18px] items-center justify-center rounded-full bg-primary mr-0.5 relative left-px"
					>
						<CheckIcon class="size-3 invert [stroke-width:3] top-px" />
					</span>
					<RadioButtonIcon v-else class="size-5 shrink-0" />
					<span
						class="min-w-0 truncate font-medium leading-5"
						:class="{ 'text-secondary line-through': step.complete }"
					>
						{{ step.label }}
					</span>
				</button>
			</div>
		</template>
	</FloatingMenu>
</template>
