<template>
	<Transition name="splash-fade" @after-leave="onAfterLeave">
		<div
			v-if="!doneLoading"
			class="splash-screen"
			:class="`${theme.active}-mode`"
			:style="splashStyle"
		>
			<div class="app-logo-wrapper" data-tauri-drag-region>
				<AppLogo class="app-logo" />
				<ProgressBar class="loading-bar" :progress="Math.min(loadingProgress, 100)" />
				<span v-if="message">{{ message }}</span>
			</div>
			<div class="gradient-bg" data-tauri-drag-region></div>
			<SplashBackground :accent="accent.effective.value" />
			<div class="base-bg"></div>
		</div>
	</Transition>
</template>

<script setup>
import { injectLoadingState } from '@orbiont/ui'
import { computed, onMounted, ref, watch } from 'vue'

import AppLogo from '@/components/ui/AppLogo.vue'
import ProgressBar from '@/components/ui/ProgressBar.vue'
import SplashBackground from '@/components/ui/SplashBackground.vue'
import { useAccent } from '@/composables/use-accent'
import { useAppEvent } from '@/composables/use-app-event'
import { useTheme } from '@/composables/use-theme.ts'
import { getAccentPalette } from '@/helpers/accent'
import { debugStartup } from '@/helpers/startup-debug'

const theme = useTheme()
const accent = useAccent()
// The splash artwork stays dark even when the interface uses the light theme.
const splashStyle = computed(() => ({
	'--color-brand': getAccentPalette(accent.effective.value, 'dark').brand,
	'--color-contrast': '#f6f8fa',
}))

const doneLoading = ref(false)
const loadingProgress = ref(0)
const message = ref()

const MIN_DISPLAY_MS = 500
const mountedAt = Date.now()

const loading = injectLoadingState()
onMounted(() => debugStartup('Splash mounted'))

function onAfterLeave() {
	debugStartup('Splash fade completed', { displayedMs: Date.now() - mountedAt })
	loading.setEnabled(true)
}

watch(
	[loading.barEnabled, loading.pending],
	([barEnabled, pending]) => {
		debugStartup('Splash loading state changed', { barEnabled, pending })
		if (barEnabled) {
			return
		}

		if (pending) {
			loadingProgress.value = 0
			fakeLoadingIncrease()
			return
		}

		const elapsed = Date.now() - mountedAt
		const delay = Math.max(0, MIN_DISPLAY_MS - elapsed)
		debugStartup('Splash dismissal scheduled', { delayMs: delay, displayedMs: elapsed })

		setTimeout(() => {
			if (loading.pending.value) {
				debugStartup('Splash dismissal deferred: new loading work')
				return
			}
			doneLoading.value = true
			debugStartup('Splash fade started', { displayedMs: Date.now() - mountedAt })
		}, delay)
	},
	{ immediate: true },
)

function fakeLoadingIncrease() {
	if (loadingProgress.value < 95) {
		setTimeout(() => {
			loadingProgress.value += 2
			fakeLoadingIncrease()
		}, 5)
	}
}

useAppEvent('loading', (e) => {
	if (e.event.type === 'directory_move') {
		loadingProgress.value = 100 * (e.fraction ?? 1)
		message.value = 'Updating app directory...'
	}
})
</script>

<style scoped lang="scss">
.splash-screen {
	position: fixed;
	inset: 0;
	z-index: 10000;
}

.splash-fade-leave-active {
	transition: opacity 0.3s ease-in-out;
}

.splash-fade-leave-to {
	opacity: 0;
}

.app-logo-wrapper {
	position: absolute;
	height: 100vh;
	width: 100%;

	display: flex;
	flex-direction: column;
	justify-content: center;
	align-items: center;

	gap: 1rem;
	color: #f6f8fa;

	z-index: 9998;
}

.app-logo {
	height: 3rem;
	width: auto;
}

.loading-bar {
	--color-button-bg: #34373e;
	max-width: 20rem;
}

.gradient-bg {
	position: absolute;
	height: 100vh;
	width: 100vw;
	background: radial-gradient(ellipse at center, rgba(0, 0, 0, 0.08), rgba(0, 0, 0, 0.2));
	z-index: 9997;
}

.base-bg {
	position: absolute;
	top: 0;
	left: 0;
	width: 100%;
	height: 100%;
	background: #16181c;
	z-index: 9995;
}
</style>
