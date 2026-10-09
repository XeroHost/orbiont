<script setup lang="ts">
import { commonMessages, defineMessages, Toggle, useVIntl } from '@orbiont/ui'

import type { DisplayPreferences } from '@/helpers/display-preferences'

defineProps<{ disabled?: boolean }>()
const model = defineModel<DisplayPreferences>({ required: true })
const { formatMessage } = useVIntl()
const messages = defineMessages({
	density: { id: 'app.appearance.layout.density', defaultMessage: 'Interface density' },
	densityDescription: {
		id: 'app.appearance.layout.density-description',
		defaultMessage: 'Adjust spacing in the library and recent instances and worlds.',
	},
	comfortable: { id: 'app.appearance.layout.comfortable', defaultMessage: 'Comfortable' },
	compact: { id: 'app.appearance.layout.compact', defaultMessage: 'Compact' },
	size: { id: 'app.appearance.layout.size', defaultMessage: 'Interface size' },
	sizeDescription: {
		id: 'app.appearance.layout.size-description',
		defaultMessage: 'Scale text and controls together. Changes apply immediately.',
	},
	small: { id: 'app.appearance.layout.small', defaultMessage: 'Small' },
	normal: { id: 'app.appearance.layout.normal', defaultMessage: 'Normal' },
	large: { id: 'app.appearance.layout.large', defaultMessage: 'Large' },
	motion: { id: 'app.appearance.layout.motion', defaultMessage: 'Reduced motion' },
	motionDescription: {
		id: 'app.appearance.layout.motion-description',
		defaultMessage: 'Reduce animations, transitions and automatic card rotation.',
	},
})
</script>

<template>
	<section class="layout-settings mt-8 border-0 border-t border-solid border-divider pt-6">
		<fieldset class="m-0 border-0 p-0" :disabled="disabled">
			<legend class="text-xl font-semibold text-contrast">
				{{ formatMessage(messages.density) }}
			</legend>
			<p class="m-0 mt-1 text-secondary">{{ formatMessage(messages.densityDescription) }}</p>
			<div class="layout-options mt-4">
				<label
					v-for="density in ['comfortable', 'compact'] as const"
					:key="density"
					:class="{ selected: model.density === density }"
				>
					<input v-model="model.density" type="radio" name="interface-density" :value="density" />
					<span>{{ formatMessage(messages[density]) }}</span>
					<span
						v-if="density === 'compact'"
						class="ml-auto shrink-0 rounded-full bg-brand-highlight px-2 py-0.5 text-xs text-brand"
						>{{ formatMessage(commonMessages.beta) }}</span
					>
				</label>
			</div>
		</fieldset>
		<fieldset class="m-0 mt-6 border-0 p-0" :disabled="disabled">
			<legend class="text-xl font-semibold text-contrast">
				{{ formatMessage(messages.size) }}
			</legend>
			<p class="m-0 mt-1 text-secondary">{{ formatMessage(messages.sizeDescription) }}</p>
			<div class="layout-options mt-4">
				<label
					v-for="size in ['small', 'normal', 'large'] as const"
					:key="size"
					:class="{ selected: model.interfaceSize === size }"
				>
					<input v-model="model.interfaceSize" type="radio" name="interface-size" :value="size" />
					<span>{{ formatMessage(messages[size]) }}</span>
				</label>
			</div>
		</fieldset>
		<div class="mt-6 flex items-center justify-between gap-4">
			<div>
				<h2 id="motion-label" class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.motion) }}
				</h2>
				<p id="motion-description" class="m-0 mt-1 text-secondary">
					{{ formatMessage(messages.motionDescription) }}
				</p>
			</div>
			<Toggle
				id="appearance-motion"
				v-model="model.reduceMotion"
				:disabled="disabled"
				aria-labelledby="motion-label"
				aria-describedby="motion-description"
			/>
		</div>
		<slot name="after-motion" />
	</section>
</template>

<style scoped>
.layout-options {
	display: flex;
	flex-wrap: wrap;
	gap: 0.625rem;
}
.layout-options label {
	display: flex;
	flex: 1;
	align-items: center;
	gap: 0.625rem;
	min-width: 7rem;
	border: 1px solid var(--surface-5);
	border-radius: 0.75rem;
	padding: 0.75rem;
	background: var(--surface-3);
	color: var(--color-contrast);
	font-weight: 600;
	cursor: pointer;
}
.layout-options label.selected {
	border-color: var(--color-brand);
	background: var(--color-button-bg-selected);
	color: var(--color-button-text-selected);
}
.layout-options label:has(input:focus-visible) {
	outline: 2px solid var(--color-focus-ring);
	outline-offset: 3px;
}
.layout-options input {
	appearance: auto !important;
	min-height: 0;
	padding: 0;
	border: 0;
	background: none;
	box-shadow: none;
	margin: 0;
	accent-color: var(--color-brand);
	flex-shrink: 0;
	width: 1rem;
	height: 1rem;
}
.layout-options label.selected input {
	accent-color: var(--color-button-text-selected);
}
</style>
