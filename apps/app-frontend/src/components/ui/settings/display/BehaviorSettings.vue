<script setup lang="ts">
import {
	defineMessages,
	injectNotificationManager,
	Toggle,
	useSavable,
	useVIntl,
} from '@orbiont/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { inject, onBeforeUnmount, onMounted } from 'vue'

import {
	DEFAULT_FEATURE_FLAGS,
	type FeatureFlag,
	useAppSettings,
} from '@/composables/use-app-settings.ts'
import {
	type AppSettings,
	appSettingsKeys,
	appSettingsQueryOptions,
	get,
	set,
} from '@/helpers/settings.ts'
import { appSettingsModalContextKey } from '@/providers/app-settings-modal'

const appSettings = useAppSettings()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const settingsModal = inject(appSettingsModalContextKey, null)
const queryClient = useQueryClient()

const compactInstanceCardsFlag: FeatureFlag = 'compact_instance_cards'
const skipNonEssentialWarningsFlag: FeatureFlag = 'skip_non_essential_warnings'
const skipUnknownPackWarningFlag: FeatureFlag = 'skip_unknown_pack_warning'
const showPlayTimeFlag: FeatureFlag = 'show_instance_play_time'
const refocusOnGameExitFlag: FeatureFlag = 'refocus_on_game_exit'

const messages = defineMessages({
	startupAndNavigationTitle: {
		id: 'app.behavior-settings.startup-and-navigation.title',
		defaultMessage: 'Startup and navigation',
	},
	contentTitle: {
		id: 'app.behavior-settings.content.title',
		defaultMessage: 'Home and content',
	},
	confirmationsTitle: {
		id: 'app.behavior-settings.confirmations.title',
		defaultMessage: 'Confirmations',
	},
	minimizeLauncherTitle: {
		id: 'app.appearance-settings.minimize-launcher.title',
		defaultMessage: 'Minimize app',
	},
	minimizeLauncherDescription: {
		id: 'app.appearance-settings.minimize-launcher.description',
		defaultMessage: 'Minimize {productName} when Minecraft starts.',
	},
	refocusLauncherTitle: {
		id: 'app.behavior-settings.refocus-launcher.title',
		defaultMessage: 'Refocus app when Minecraft closes',
	},
	refocusLauncherDescription: {
		id: 'app.behavior-settings.refocus-launcher.description',
		defaultMessage: 'Bring {productName} to the foreground when Minecraft closes.',
	},
	defaultLandingPageHome: {
		id: 'app.appearance-settings.default-landing-page.home',
		defaultMessage: 'Home',
	},
	defaultLandingPageLibrary: {
		id: 'app.appearance-settings.default-landing-page.library',
		defaultMessage: 'Library',
	},
	compactModeTitle: {
		id: 'app.appearance-settings.compact-mode.title',
		defaultMessage: 'Compact mode',
	},
	compactModeDescription: {
		id: 'app.appearance-settings.compact-mode.description',
		defaultMessage: 'Display library instances in a compact row layout.',
	},
	showPlayTimeTitle: {
		id: 'app.appearance-settings.show-play-time.title',
		defaultMessage: 'Show play time',
	},
	showPlayTimeDescription: {
		id: 'app.appearance-settings.show-play-time.description',
		defaultMessage: `Show how long you've played each instance.`,
	},
	hideNametagTitle: {
		id: 'app.appearance-settings.hide-nametag.title',
		defaultMessage: 'Hide nametag',
	},
	hideNametagDescription: {
		id: 'app.appearance-settings.hide-nametag.description',
		defaultMessage: 'Hide your username above the player preview on the Skin selector page.',
	},
	unknownPackWarningTitle: {
		id: 'app.appearance-settings.unknown-pack-warning.title',
		defaultMessage: 'Warn me before installing unknown modpacks',
	},
	unknownPackWarningDescription: {
		id: 'app.appearance-settings.unknown-pack-warning.imported-description',
		defaultMessage:
			'Show a safety warning before installing an imported modpack (.mrpack or CurseForge .zip) whose source could not be verified.',
	},
	skipNonEssentialWarningsTitle: {
		id: 'app.appearance-settings.skip-non-essential-warnings.title',
		defaultMessage: 'Skip non-essential warnings',
	},
	skipNonEssentialWarningsDescription: {
		id: 'app.appearance-settings.skip-non-essential-warnings.description',
		defaultMessage:
			'Skip confirmations for low-risk actions such as duplicate installs, normal content deletion, bulk updates, unlinking, and repairs. Warnings for dangerous actions are always shown.',
	},
})

type BehaviorSettingsState = {
	minimizeApp: boolean
	refocusApp: boolean
	compactInstanceCards: boolean
	showPlayTime: boolean
	hideNametag: boolean
	warnOnUnknownModpacks: boolean
	skipNonEssentialWarnings: boolean
}

const settingsQuery = useQuery(appSettingsQueryOptions())
await settingsQuery.suspense()

function getBehaviorSettingsState(settings: AppSettings): BehaviorSettingsState {
	return {
		minimizeApp: settings.hide_on_process_start,
		refocusApp:
			settings.feature_flags[refocusOnGameExitFlag] ?? DEFAULT_FEATURE_FLAGS[refocusOnGameExitFlag],
		compactInstanceCards:
			settings.feature_flags[compactInstanceCardsFlag] ??
			DEFAULT_FEATURE_FLAGS[compactInstanceCardsFlag],
		showPlayTime:
			settings.feature_flags[showPlayTimeFlag] ?? DEFAULT_FEATURE_FLAGS[showPlayTimeFlag],
		hideNametag: settings.hide_nametag_skins_page,
		warnOnUnknownModpacks: !(
			settings.feature_flags[skipUnknownPackWarningFlag] ??
			DEFAULT_FEATURE_FLAGS[skipUnknownPackWarningFlag]
		),
		skipNonEssentialWarnings:
			settings.feature_flags[skipNonEssentialWarningsFlag] ??
			DEFAULT_FEATURE_FLAGS[skipNonEssentialWarningsFlag],
	}
}

const settingsMutation = useMutation({
	mutationKey: appSettingsKeys.update,
	scope: { id: 'app-settings' },
	mutationFn: async (value: BehaviorSettingsState) => {
		const latestSettings = await get()
		const nextSettings: AppSettings = {
			...latestSettings,
			hide_on_process_start: value.minimizeApp,
			hide_nametag_skins_page: value.hideNametag,
			feature_flags: {
				...latestSettings.feature_flags,
				[refocusOnGameExitFlag]: value.refocusApp,
				[compactInstanceCardsFlag]: value.compactInstanceCards,
				[showPlayTimeFlag]: value.showPlayTime,
				[skipUnknownPackWarningFlag]: !value.warnOnUnknownModpacks,
				[skipNonEssentialWarningsFlag]: value.skipNonEssentialWarnings,
			},
		}

		await set(nextSettings)
		queryClient.setQueryData(appSettingsKeys.all, nextSettings)
		appSettings.hideNametagSkinsPage = value.hideNametag
		appSettings.featureFlags[refocusOnGameExitFlag] = value.refocusApp
		appSettings.featureFlags[compactInstanceCardsFlag] = value.compactInstanceCards
		appSettings.featureFlags[showPlayTimeFlag] = value.showPlayTime
		appSettings.featureFlags[skipUnknownPackWarningFlag] = !value.warnOnUnknownModpacks
		appSettings.featureFlags[skipNonEssentialWarningsFlag] = value.skipNonEssentialWarnings
	},
	onMutate: () => queryClient.cancelQueries({ queryKey: appSettingsKeys.all }),
	onError: handleError,
	onSettled: () => queryClient.invalidateQueries({ queryKey: appSettingsKeys.all }),
})

const { saved, current, changes, saving, hasChanges, reset, save } =
	useSavable<BehaviorSettingsState>(
		() => getBehaviorSettingsState(settingsQuery.data.value!),
		(): Promise<void> => settingsMutation.mutateAsync({ ...current.value }),
	)

async function saveBehaviorSettings(): Promise<void> {
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
		save: saveBehaviorSettings,
	})
})

onBeforeUnmount(() => {
	settingsModal?.registerUnsavedChangesController(null)
})
</script>
<template>
	<section>
		<h2 class="m-0 text-xl font-semibold text-contrast">
			{{ formatMessage(messages.startupAndNavigationTitle) }}
		</h2>
		<div class="mt-4 flex flex-col gap-6">
			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.minimizeLauncherTitle) }}
					</h3>
					<p class="m-0 mt-1">
						{{ formatMessage(messages.minimizeLauncherDescription) }}
					</p>
				</div>
				<Toggle id="minimize-launcher" v-model="current.minimizeApp" />
			</div>
			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 id="refocus-launcher-label" class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.refocusLauncherTitle) }}
					</h3>
					<p id="refocus-launcher-description" class="m-0 mt-1">
						{{ formatMessage(messages.refocusLauncherDescription) }}
					</p>
				</div>
				<Toggle
					id="refocus-launcher"
					v-model="current.refocusApp"
					aria-labelledby="refocus-launcher-label"
					aria-describedby="refocus-launcher-description"
				/>
			</div>
		</div>
	</section>

	<section class="mt-8 border-0 border-t border-solid border-divider pt-6">
		<h2 class="m-0 text-xl font-semibold text-contrast">
			{{ formatMessage(messages.contentTitle) }}
		</h2>
		<div class="mt-4 flex flex-col gap-6">
			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.compactModeTitle) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.compactModeDescription) }}</p>
				</div>
				<Toggle id="compact-mode" v-model="current.compactInstanceCards" />
			</div>

			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.showPlayTimeTitle) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.showPlayTimeDescription) }}</p>
				</div>
				<Toggle id="show-play-time" v-model="current.showPlayTime" />
			</div>

			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.hideNametagTitle) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.hideNametagDescription) }}</p>
				</div>
				<Toggle id="hide-nametag-skins-page" v-model="current.hideNametag" />
			</div>
		</div>
	</section>

	<section class="mt-8 border-0 border-t border-solid border-divider pt-6">
		<h2 class="m-0 text-xl font-semibold text-contrast">
			{{ formatMessage(messages.confirmationsTitle) }}
		</h2>
		<div class="mt-4 flex flex-col gap-6">
			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.unknownPackWarningTitle) }}
					</h3>
					<p class="m-0 mt-1">
						{{ formatMessage(messages.unknownPackWarningDescription) }}
					</p>
				</div>
				<Toggle
					id="warn-before-installing-unknown-modpacks"
					v-model="current.warnOnUnknownModpacks"
				/>
			</div>

			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.skipNonEssentialWarningsTitle) }}
					</h3>
					<p class="m-0 mt-1">
						{{ formatMessage(messages.skipNonEssentialWarningsDescription) }}
					</p>
				</div>
				<Toggle id="skip-non-essential-warnings" v-model="current.skipNonEssentialWarnings" />
			</div>
		</div>
	</section>
</template>
