<template>
	<component
		:is="isLink ? RouterLink : 'button'"
		v-tooltip.right="showLabel ? undefined : (tooltip ?? label)"
		data-animated-icon-trigger
		:to="isLink ? to : undefined"
		v-bind="$attrs"
		:disabled="isLink ? undefined : disabled"
		:active-class="isLink && isSubpage ? '' : undefined"
		:class="{
			'router-link-active': isPrimary && isPrimary(route),
			'subpage-active': isSubpage && isSubpage(route),
			disabled: disabled,
			'nav-button--labeled': showLabel,
		}"
		class="nav-button border-none text-primary cursor-pointer h-12 flex items-center text-2xl transition-all bg-transparent hover:bg-button-bg hover:text-contrast"
		:aria-label="showLabel ? undefined : label"
		@click="onClick"
	>
		<span
			class="flex shrink-0 items-center justify-center"
			:class="showLabel ? 'size-11' : 'size-12'"
		>
			<slot />
		</span>
		<span v-if="showLabel" class="min-w-0 truncate pr-3 text-base font-semibold">
			{{ label }}
		</span>
	</component>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { RouteLocationNormalizedLoaded } from 'vue-router'
import { RouterLink, useRoute } from 'vue-router'

import { useNavExpanded } from '@/composables/use-nav-expanded'

const route = useRoute()
const { navExpanded } = useNavExpanded()

type RouteFunction = (route: RouteLocationNormalizedLoaded) => boolean

const props = withDefaults(
	defineProps<{
		to: (() => void) | string
		/** Shown next to the icon when the nav is expanded, and as the tooltip when collapsed. */
		label?: string
		/** Collapsed-state tooltip, when it should differ from the label. */
		tooltip?: string
		isPrimary?: RouteFunction
		isSubpage?: RouteFunction
		highlightOverride?: boolean
		disabled?: boolean
	}>(),
	{
		disabled: false,
	},
)

const isLink = computed(() => typeof props.to === 'string')
const showLabel = computed(() => navExpanded.value && !!props.label)

function onClick() {
	if (typeof props.to === 'function') {
		props.to()
	}
}

defineOptions({
	inheritAttrs: false,
})
</script>

<style lang="scss" scoped>
.nav-button {
	position: relative;
	width: 3rem;
	border-radius: 9999px;

	&::before {
		content: '';
		position: absolute;
		inset: 0;
		background-color: var(--color-button-bg-selected);
		border-radius: inherit;
		opacity: 0;
		scale: 0.4;
		z-index: -1;
		transition:
			opacity 0.25s var(--ease-out-expo),
			scale 0.25s var(--ease-out-expo);
	}
}

.nav-button--labeled {
	width: 100%;
	height: 2.75rem;
	border-radius: var(--radius-lg);
}

.router-link-active,
.subpage-active {
	svg {
		filter: drop-shadow(0 0 0.5rem black);
	}
}

.router-link-active {
	@apply text-[--color-button-text-selected];
	&::before {
		background-color: var(--color-button-bg-selected);
		scale: 1;
		opacity: 1;
	}
}

.subpage-active {
	@apply text-contrast bg-button-bg;
}
</style>
