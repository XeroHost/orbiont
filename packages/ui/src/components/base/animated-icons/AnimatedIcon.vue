<script setup lang="ts">
import { type AnimatedIconName, hugeIcons, partMotion } from './registry'

defineProps<{ name: AnimatedIconName }>()

// Hugeicons data uses React attribute names; normalize them for native SVG.
function svgAttributes(attributes: Readonly<Record<string, string | number>>) {
	return Object.fromEntries(
		Object.entries(attributes)
			.filter(([key]) => key !== 'key')
			.map(([key, value]) => [
				key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`),
				value,
			]),
	)
}
</script>

<template>
	<svg
		class="huge-animated-icon"
		:data-huge-icon="name"
		width="1em"
		height="1em"
		viewBox="0 0 24 24"
		fill="none"
		aria-hidden="true"
		focusable="false"
	>
		<component
			:is="tag"
			v-for="([tag, attributes], index) in hugeIcons[name]"
			:key="attributes.key"
			v-bind="svgAttributes(attributes)"
			class="huge-icon-part"
			:pathLength="1"
			:style="{
				'--icon-motion':
					partMotion(name, index) === 'none' ? 'none' : `huge-${partMotion(name, index)}`,
				'--icon-delay': `${(index % 3) * 40}ms`,
			}"
		/>
	</svg>
</template>

<style>
.huge-animated-icon {
	display: inline-block;
	flex-shrink: 0;
	overflow: visible;
	vertical-align: middle;
}
.huge-icon-part {
	transform-box: fill-box;
	transform-origin: center;
}
.huge-animated-icon[data-huge-icon='compass'] .huge-icon-part {
	transform-box: view-box;
	transform-origin: 12px 13px;
}
.huge-animated-icon[data-huge-icon='chart'] .huge-icon-part {
	transform-origin: center bottom;
}

/* The whole control triggers its icon, including keyboard focus. No loops. */
:is([data-animated-icon-trigger], button, a, [role='button'], [role='menuitem']):focus-visible:not(
		:disabled
	):not([aria-disabled='true'])
	.huge-icon-part {
	animation: var(--icon-motion) 650ms ease-in-out var(--icon-delay) 1;
}
@media (hover: hover) {
	:is([data-animated-icon-trigger], button, a, [role='button'], [role='menuitem']):hover:not(
			:disabled
		):not([aria-disabled='true'])
		.huge-icon-part {
		animation: var(--icon-motion) 650ms ease-in-out var(--icon-delay) 1;
	}
}
@media (prefers-reduced-motion: reduce) {
	:root:not([data-icon-motion='on']) .huge-icon-part {
		animation: none !important;
	}
}
:root[data-icon-motion='off'] .huge-icon-part {
	animation: none !important;
}
@keyframes huge-needle {
	0%,
	100% {
		transform: rotate(0);
	}
	30% {
		transform: rotate(45deg);
	}
	60% {
		transform: rotate(-20deg);
	}
	80% {
		transform: rotate(8deg);
	}
}
@keyframes huge-steam {
	0%,
	100% {
		transform: translateY(0);
		opacity: 1;
	}
	45% {
		transform: translateY(-2px);
		opacity: 0.35;
	}
}
@keyframes huge-spark {
	0%,
	100% {
		transform: scale(1);
		opacity: 1;
	}
	35% {
		transform: scale(0.75);
		opacity: 0.5;
	}
	65% {
		transform: scale(1.12);
		opacity: 1;
	}
}
@keyframes huge-gear {
	0% {
		transform: rotate(0);
	}
	65% {
		transform: rotate(70deg);
	}
	100% {
		transform: rotate(0);
	}
}
@keyframes huge-slide {
	0%,
	100% {
		transform: translateX(0);
	}
	40% {
		transform: translateX(2px);
	}
	75% {
		transform: translateX(-1px);
	}
}
@keyframes huge-sword-left {
	0%,
	100% {
		transform: rotate(0);
	}
	45% {
		transform: rotate(-12deg);
	}
	70% {
		transform: rotate(4deg);
	}
}
@keyframes huge-sword-right {
	0%,
	100% {
		transform: rotate(0);
	}
	45% {
		transform: rotate(12deg);
	}
	70% {
		transform: rotate(-4deg);
	}
}
@keyframes huge-bar {
	0%,
	100% {
		transform: scaleY(1);
	}
	40% {
		transform: scaleY(0.5);
	}
	70% {
		transform: scaleY(1.1);
	}
}
@keyframes huge-wand {
	0%,
	100% {
		transform: rotate(0);
	}
	40% {
		transform: rotate(-12deg);
	}
	70% {
		transform: rotate(6deg);
	}
}
@keyframes huge-advance {
	0%,
	100% {
		transform: translateX(0);
	}
	40% {
		transform: translateX(2px);
	}
	70% {
		transform: translateX(-1px);
	}
}
@keyframes huge-sway {
	0%,
	100% {
		transform: rotate(0);
	}
	30% {
		transform: rotate(-9deg);
	}
	65% {
		transform: rotate(7deg);
	}
}
@keyframes huge-lift {
	0%,
	100% {
		transform: translateY(0);
	}
	45% {
		transform: translateY(-1.5px);
	}
}
@keyframes huge-draw {
	0% {
		stroke-dasharray: 1;
		stroke-dashoffset: 1;
	}
	100% {
		stroke-dasharray: 1;
		stroke-dashoffset: 0;
	}
}
</style>
