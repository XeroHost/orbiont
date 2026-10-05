<script setup lang="ts">
import { ExternalIcon } from '@orbiont/assets'
import { companyName, companySiteUrl } from '@orbiont/branding'
import { defineMessages, useVIntl } from '@orbiont/ui'

import companyBackdrop from '@/assets/branding/xerohost-games.png'
import companyLogo from '@/assets/branding/xerohost-icon.png'

defineEmits<{ visit: [] }>()

const { formatMessage } = useVIntl()
const messages = defineMessages({
	headline: {
		id: 'app.sidebar.hosting.headline',
		defaultMessage: 'Your next Minecraft server',
	},
	action: {
		id: 'app.sidebar.hosting.action',
		defaultMessage: 'Explore hosting',
	},
})
</script>

<template>
	<section class="hosting-promotion" :aria-label="companyName">
		<a
			class="hosting-card no-click-animation"
			:href="companySiteUrl"
			target="_blank"
			rel="noopener noreferrer"
			@click.stop.prevent="$emit('visit')"
		>
			<img :src="companyBackdrop" alt="" class="hosting-backdrop" />
			<div class="hosting-wordmark">
				<img :src="companyLogo" alt="" class="hosting-logo" />
				<span>{{ companyName }}</span>
			</div>
			<p class="hosting-headline">{{ formatMessage(messages.headline) }}</p>
			<span class="hosting-action">
				{{ formatMessage(messages.action) }}
				<ExternalIcon aria-hidden="true" />
			</span>
		</a>
	</section>
</template>

<style scoped lang="scss">
.hosting-promotion {
	padding: 0;
	flex-shrink: 0;
}

.hosting-card {
	display: flex;
	flex-direction: column;
	gap: 16px;
	padding: 20px;
	border: 0;
	position: relative;
	overflow: hidden;
	isolation: isolate;
	border-radius: 0;
	width: 100%;
	box-sizing: border-box;
	color: var(--color-contrast);
	text-decoration: none;
	background:
		radial-gradient(ellipse at top right, var(--brand-gradient-button), transparent 75%),
		var(--color-raised-bg);
	transform: none;
	transition: none;

	&:active {
		transform: none;
	}

	&:focus-visible {
		outline: 2px solid var(--color-brand);
		outline-offset: -2px;
	}
}

.hosting-backdrop {
	position: absolute;
	inset: auto 0 0;
	width: 100%;
	height: 65%;
	object-fit: cover;
	object-position: center bottom;
	opacity: 0.35;
	mask-image: linear-gradient(to bottom, transparent, black 80%);
	pointer-events: none;
	z-index: -1;
}

.hosting-wordmark {
	display: flex;
	align-items: center;
	gap: 10px;
	font-size: 20px;
	font-weight: 800;

	.hosting-logo {
		object-fit: contain;
		width: 28px;
		height: 28px;
		color: var(--color-brand);
	}
}

.hosting-headline {
	margin: 0;
	max-width: 240px;
	font-size: 22px;
	font-weight: 700;
	line-height: 1.25;
	text-wrap: balance;
}

.hosting-action {
	display: flex;
	align-items: center;
	justify-content: space-between;
	gap: 8px;
	font-size: 13px;
	font-weight: 600;
	color: var(--color-secondary);

	svg {
		width: 16px;
		height: 16px;
	}
}

@media (prefers-reduced-motion: reduce) {
	.hosting-card {
		transition: none;
	}
}
</style>
