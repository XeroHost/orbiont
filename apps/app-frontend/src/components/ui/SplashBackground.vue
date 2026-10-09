<script setup lang="ts">
import { computed, useId } from 'vue'

import backgroundUrl from '@/assets/branding/orbiont-splash-background.png'
import type { Accent } from '@/helpers/accent'
import { getSplashAccentMatrix } from '@/helpers/splash-accent'

const props = defineProps<{ accent: Accent }>()
const filterId = `splash-accent-${useId()}`
const matrix = computed(() => getSplashAccentMatrix(props.accent))
const backgroundStyle = computed(() => ({
	backgroundImage: `url(${backgroundUrl})`,
	filter: matrix.value ? `url("#${filterId}")` : 'none',
}))
</script>

<template>
	<svg class="tint-definitions" aria-hidden="true" focusable="false" width="0" height="0">
		<defs>
			<filter
				:id="filterId"
				x="0"
				y="0"
				width="100%"
				height="100%"
				color-interpolation-filters="sRGB"
			>
				<feColorMatrix type="matrix" :values="matrix ?? undefined" />
			</filter>
		</defs>
	</svg>
	<div class="image-bg" :style="backgroundStyle" aria-hidden="true"></div>
</template>

<style scoped>
.tint-definitions {
	position: absolute;
	pointer-events: none;
}

.image-bg {
	position: absolute;
	inset: 0;
	background-color: #16181c;
	background-position: center;
	background-size: cover;
	background-repeat: no-repeat;
	z-index: 9996;
}
</style>
