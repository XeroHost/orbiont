<script setup lang="ts">
import { CheckIcon, RadioButtonIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import { injectOnboardingChecklist } from '@/providers/onboarding-checklist'

// Lives inside the account menu in the top bar; `done` lets that menu close
// after a step's action starts.
const emit = defineEmits<{
	'create-instance': []
	'login-minecraft': []
	done: []
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

/** Whether there's anything to show: the checklist is on and a step is left. */
const visible = computed(
	() => isReady.value && showChecklist.value && steps.value.some((step) => !step.complete),
)
</script>

<template>
	<div
		v-if="visible"
		class="flex flex-col gap-2 border-0 border-t border-solid border-surface-5 px-1 pt-2"
	>
		<span class="text-xs font-medium uppercase text-secondary">
			{{ formatMessage(messages.title) }}
		</span>
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
					emit('done')
					step.action()
				}
			"
		>
			<span
				v-if="step.complete"
				class="relative left-px mr-0.5 flex size-[18px] items-center justify-center rounded-full bg-primary"
			>
				<CheckIcon class="top-px size-3 invert [stroke-width:3]" />
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
