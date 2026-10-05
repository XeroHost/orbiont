<script setup lang="ts">
import { MegaphoneIcon } from '@orbiont/assets'
import { defineMessages, useVIntl } from '@orbiont/ui'

// Development layout preview only. Live Nitro inventory needs approval for
// Tauri and a separate, isolated webview integration before it can be enabled.
const props = defineProps<{ count: 1 | 2 }>()
const preview = import.meta.env.DEV
const placements = ['sidebar-upper', 'sidebar-lower'] as const
const { formatMessage } = useVIntl()
const messages = defineMessages({
	advertisement: {
		id: 'app.sidebar.advertising.label',
		defaultMessage: 'Advertisement',
	},
	preview: {
		id: 'app.sidebar.advertising.preview',
		defaultMessage: 'Preview · placement {number}',
	},
})
</script>

<template>
	<div
		v-if="preview"
		class="sidebar-ad-placements"
		role="group"
		:aria-label="formatMessage(messages.advertisement)"
		data-ad-preview
	>
		<section
			v-for="(placement, index) in placements.slice(0, props.count)"
			:key="placement"
			class="ad-placement"
			:data-ad-placement="placement"
			:aria-label="formatMessage(messages.preview, { number: index + 1 })"
		>
			<div class="ad-preview">
				<MegaphoneIcon aria-hidden="true" />
				<span>{{ formatMessage(messages.preview, { number: index + 1 }) }}</span>
			</div>
		</section>
	</div>
</template>

<style scoped lang="scss">
.sidebar-ad-placements {
	display: flex;
	flex-direction: column;
	gap: 0;
	width: 100%;
	margin-top: auto;
	flex-shrink: 0;
	position: relative;
}

.ad-placement {
	display: flex;
	flex-direction: column;
	width: 100%;

	& + .ad-placement .ad-preview {
		border-top: 0;
	}
}

.ad-preview {
	box-sizing: border-box;
	display: flex;
	flex-direction: column;
	align-items: center;
	justify-content: center;
	gap: 12px;
	width: 100%;
	height: 250px;
	border: 1px solid var(--brand-gradient-border);
	border-radius: 0;
	background: var(--brand-gradient-button);
	color: var(--color-secondary);
	font-size: 13px;

	svg {
		width: 28px;
		height: 28px;
		opacity: 0.6;
	}
}
</style>
