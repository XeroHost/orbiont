<script setup lang="ts">
import { CheckIcon, PlayIcon } from '@orbiont/assets'
import { commonMessages, defineMessages, useVIntl } from '@orbiont/ui'

import AppLogo from '@/components/ui/AppLogo.vue'
import { useTheme } from '@/composables/use-theme'
import { type Accent, ACCENT_OPTIONS, DEFAULT_ACCENT, getAccentPalette } from '@/helpers/accent'

defineProps<{ disabled?: boolean }>()
const model = defineModel<Accent>({ required: true })
const theme = useTheme()
const { formatMessage } = useVIntl()
const messages = defineMessages({
	title: { id: 'app.appearance.accent.title', defaultMessage: 'Accent color' },
	description: {
		id: 'app.appearance.accent.description',
		defaultMessage: 'Personalize the logo, buttons and selections independently of your theme.',
	},
	color: {
		id: 'app.appearance.accent.color',
		defaultMessage:
			'{color, select, cyan {{productName} cyan} sky {Sky blue} blue {Blue} indigo {Indigo} violet {Violet} pink {Pink} red {Red} coral {Coral} orange {Orange} amber {Amber} yellow {Yellow} lime {Lime} green {Green} emerald {Emerald} teal {Teal} other {Color}}',
	},
	preview: { id: 'app.appearance.accent.preview', defaultMessage: 'Preview' },
	reset: { id: 'app.appearance.accent.reset', defaultMessage: 'Restore default color' },
})
</script>

<template>
	<section
		class="mt-8 border-0 border-t border-solid border-divider pt-6"
		aria-labelledby="accent-title"
	>
		<h2 id="accent-title" class="m-0 text-xl font-semibold text-contrast">
			{{ formatMessage(messages.title) }}
		</h2>
		<p class="m-0 mt-1 text-secondary">{{ formatMessage(messages.description) }}</p>
		<fieldset class="accent-options m-0 mt-4 border-0 p-0" :disabled="disabled">
			<legend class="sr-only">{{ formatMessage(messages.title) }}</legend>
			<label
				v-for="color in ACCENT_OPTIONS"
				:key="color"
				class="accent-option"
				:class="{ selected: model === color }"
			>
				<input
					v-model="model"
					type="radio"
					name="orbiont-accent"
					:value="color"
					class="accent-radio"
				/>
				<span
					class="accent-swatch"
					:style="{ background: getAccentPalette(color, theme.active).brand }"
					aria-hidden="true"
				>
					<CheckIcon
						v-if="model === color"
						class="size-5"
						:style="{
							color: theme.active === 'light' || theme.active === 'retro' ? '#ffffff' : '#000000',
						}"
					/>
				</span>
				<span>{{ formatMessage(messages.color, { color }) }}</span>
			</label>
		</fieldset>
		<div
			class="accent-preview mt-4 rounded-2xl bg-surface-1 p-4"
			:aria-label="formatMessage(messages.preview)"
		>
			<div class="flex flex-wrap items-center justify-between gap-4">
				<AppLogo />
				<span class="text-sm text-secondary">{{ formatMessage(messages.preview) }}</span>
			</div>
			<div class="mt-4 flex items-center justify-between gap-3">
				<span class="accent-preview-tab rounded-xl px-3 py-2 font-semibold">{{
					formatMessage(commonMessages.selectedLabel)
				}}</span>
				<span
					class="accent-preview-button inline-flex items-center gap-2 rounded-xl px-4 py-2 font-semibold"
					><PlayIcon class="size-5" />{{ formatMessage(commonMessages.playButton) }}</span
				>
			</div>
			<div class="mt-4 h-2 overflow-hidden rounded-full bg-surface-4" aria-hidden="true">
				<div class="h-full w-3/5 rounded-full bg-brand" />
			</div>
		</div>
		<button
			type="button"
			class="accent-reset mt-3"
			:disabled="disabled || model === DEFAULT_ACCENT"
			@click="model = DEFAULT_ACCENT"
		>
			{{ formatMessage(messages.reset) }}
		</button>
	</section>
</template>

<style scoped>
.accent-options {
	display: grid;
	grid-template-columns: repeat(5, minmax(0, 1fr));
	gap: 0.625rem;
}
.accent-option {
	position: relative;
	display: flex;
	flex-direction: column;
	align-items: center;
	gap: 0.625rem;
	padding: 0.875rem 0.5rem;
	border: 1px solid var(--surface-5);
	border-radius: 0.875rem;
	background: var(--surface-3);
	color: var(--color-contrast);
	font-size: 0.875rem;
	font-weight: 600;
	text-align: center;
	overflow-wrap: anywhere;
	cursor: pointer;
}
.accent-option.selected {
	border-color: var(--color-brand);
	background: var(--color-button-bg-selected);
	color: var(--color-button-text-selected);
}
.accent-option:not(.selected):hover {
	background: var(--surface-4);
}
.accent-option:focus-within {
	outline: 2px solid var(--color-focus-ring);
	outline-offset: 3px;
}
.accent-radio {
	position: absolute;
	inset: 0;
	width: 100%;
	height: 100%;
	margin: 0;
	opacity: 0;
	cursor: pointer;
}
.accent-swatch {
	display: flex;
	align-items: center;
	justify-content: center;
	width: 2.5rem;
	height: 2.5rem;
	border-radius: 50%;
	pointer-events: none;
}
.accent-preview {
	border: 1px solid var(--surface-5);
}
.accent-preview-tab {
	background: var(--color-button-bg-selected);
	color: var(--color-button-text-selected);
}
.accent-preview-button {
	background: var(--color-brand);
	color: var(--color-accent-contrast);
}
.accent-reset {
	border: 0;
	border-radius: 0.5rem;
	padding: 0.5rem 0;
	background: transparent;
	color: var(--color-contrast);
	font: inherit;
	font-size: 0.875rem;
	text-decoration: underline;
	text-underline-offset: 3px;
	cursor: pointer;
}
.accent-reset:focus-visible {
	outline: 2px solid var(--color-focus-ring);
	outline-offset: 3px;
}
.accent-reset:disabled {
	opacity: 0.5;
	cursor: default;
}
.accent-options:disabled .accent-option {
	opacity: 0.5;
	cursor: default;
}

@media (max-width: 480px) {
	.accent-options {
		grid-template-columns: repeat(3, minmax(0, 1fr));
	}
}
</style>
