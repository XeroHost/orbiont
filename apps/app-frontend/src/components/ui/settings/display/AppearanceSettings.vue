<script setup lang="ts">
import {
	AppearanceSettingsLayout,
	defineMessages,
	provideAppearanceSettings,
	Toggle,
	useSavable,
	useVIntl,
} from '@orbiont/ui'
import { platform } from '@tauri-apps/plugin-os'
import { computed, inject, onBeforeUnmount, onMounted, watch } from 'vue'

import { useAppSettings } from '@/composables/use-app-settings.ts'
import { type IconMotion, useIconMotion } from '@/composables/use-icon-motion'
import { type ColorTheme, isDarkTheme, useTheme } from '@/composables/use-theme.ts'
import { type AppSettings, get, set } from '@/helpers/settings.ts'
import { appSettingsModalContextKey } from '@/providers/app-settings-modal'

const theme = useTheme()
const appSettings = useAppSettings()
const settingsModal = inject(appSettingsModalContextKey, null)
const os = platform()
const iconMotion = useIconMotion()
const { formatMessage } = useVIntl()
const messages = defineMessages({
	iconMotionTitle: {
		id: 'app.appearance-settings.icon-motion.title',
		defaultMessage: 'Icon animations',
	},
	iconMotionDescription: {
		id: 'app.appearance-settings.icon-motion.description',
		defaultMessage: 'Animate icons on hover and keyboard focus.',
	},
})

type AppearanceSettingsState = {
	theme: ColorTheme
	advancedRendering: boolean
	nativeDecorations: boolean
	iconMotion: IconMotion
}

function getAppearanceSettingsState(): AppearanceSettingsState {
	return {
		theme: theme.preferred,
		advancedRendering: theme.advancedRendering,
		nativeDecorations: appSettings.nativeDecorations,
		iconMotion: iconMotion.current.value,
	}
}

const { saved, current, changes, saving, hasChanges, reset, save } = useSavable(
	getAppearanceSettingsState,
	async () => {
		const value = current.value
		const nextSettings: AppSettings = {
			...(await get()),
			theme: value.theme,
			advanced_rendering: value.advancedRendering,
			native_decorations: value.nativeDecorations,
		}

		await set(nextSettings)
		if (isDarkTheme(value.theme)) {
			theme.preferredDark = value.theme
		}
		theme.preferred = value.theme
		theme.advancedRendering = value.advancedRendering
		appSettings.nativeDecorations = value.nativeDecorations
		iconMotion.set(value.iconMotion)
	},
)

const themeOptions = computed(() =>
	theme.options.filter(
		(option) => option !== 'retro' || appSettings.devMode || current.value.theme === 'retro',
	),
)

const iconMotionEnabled = computed({
	get: () => current.value.iconMotion === 'on',
	set: (enabled: boolean) => {
		current.value.iconMotion = enabled ? 'on' : 'off'
	},
})

const preferredDarkTheme = computed(() =>
	isDarkTheme(current.value.theme) ? current.value.theme : theme.preferredDark,
)

function setTheme(value: ColorTheme): void {
	current.value.theme = value
}

function setAdvancedRendering(enabled: boolean): void {
	current.value.advancedRendering = enabled
}

function setNativeDecorations(enabled: boolean): void {
	current.value.nativeDecorations = enabled
}

watch(
	() => current.value.iconMotion,
	(value) => {
		iconMotion.preview.value = value
	},
	{ immediate: true },
)

watch(
	[() => current.value.theme, () => saved.value.theme],
	([selectedTheme, savedTheme]) => {
		theme.preview = selectedTheme === savedTheme ? null : selectedTheme
	},
	{ immediate: true },
)

async function saveAppearanceSettings(): Promise<void> {
	try {
		await save()
	} catch {
		return
	}
}

onMounted(() => {
	settingsModal?.registerUnsavedChangesController({
		hasChanges: () => hasChanges.value,
		getOriginal: () => saved.value,
		getModified: () => changes.value,
		isSaving: () => saving.value,
		reset,
		save: saveAppearanceSettings,
	})
})

onBeforeUnmount(() => {
	iconMotion.preview.value = null
	theme.preview = null
	settingsModal?.registerUnsavedChangesController(null)
})

provideAppearanceSettings({
	deferPersistence: true,
	theme: {
		current: computed(() => current.value.theme),
		options: themeOptions,
		system: computed(() => (theme.native === 'light' ? 'light' : preferredDarkTheme.value)),
		preferredDark: preferredDarkTheme,
		set: setTheme,
	},
	advancedRendering: {
		value: computed(() => current.value.advancedRendering),
		set: setAdvancedRendering,
	},
	nativeDecorations:
		os !== 'macos'
			? {
					value: computed(() => current.value.nativeDecorations),
					set: setNativeDecorations,
				}
			: undefined,
})
</script>

<template>
	<div>
		<AppearanceSettingsLayout />
		<section
			class="mt-8 flex items-center justify-between gap-4 border-0 border-t border-solid border-divider pt-6"
		>
			<div>
				<h2 id="icon-motion-label" class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.iconMotionTitle) }}
				</h2>
				<p id="icon-motion-description" class="m-0 mt-1 text-secondary">
					{{ formatMessage(messages.iconMotionDescription) }}
				</p>
			</div>
			<Toggle
				id="icon-motion"
				v-model="iconMotionEnabled"
				aria-labelledby="icon-motion-label"
				aria-describedby="icon-motion-description"
			/>
		</section>
	</div>
</template>
