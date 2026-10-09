<script setup>
import { ApiError, TauriApiClient, VerboseLoggingFeature } from '@orbiont/api-client'
import { ChevronLeftIcon, ChevronRightIcon, PanelLeftIcon, RefreshCwIcon } from '@orbiont/assets'
import { changelogUrl, companySiteUrl, productName, supportEmail } from '@orbiont/branding'
import {
	Admonition,
	AnimatedIcon,
	commonMessages,
	ContentInstallModal,
	ContentUpdaterModal,
	CreationFlowModal,
	defineMessages,
	I18nDebugPanel,
	IconButton,
	LoadingBar,
	NotificationPanel,
	PopupNotificationPanel,
	provideApiClient,
	provideModalBehavior,
	provideNotificationManager,
	providePageContext,
	providePopupNotificationManager,
	TooltipDirective,
	useDebugLogger,
	useFormatBytes,
	useVIntl,
} from '@orbiont/ui'
import { useQueries, useQuery, useQueryClient } from '@tanstack/vue-query'
import { getVersion } from '@tauri-apps/api/app'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { openUrl } from '@tauri-apps/plugin-opener'
import { type } from '@tauri-apps/plugin-os'
import { saveWindowState, StateFlags } from '@tauri-apps/plugin-window-state'
import { computed, nextTick, onMounted, onUnmounted, provide, ref, watch } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'

import AccountsCard from '@/components/ui/AccountsCard.vue'
import AppActionBar from '@/components/ui/AppActionBar.vue'
import AppLogo from '@/components/ui/AppLogo.vue'
import Breadcrumbs from '@/components/ui/Breadcrumbs.vue'
import ErrorModal from '@/components/ui/ErrorModal.vue'
import BedrockInstallModal from '@/components/ui/install_flow/BedrockInstallModal.vue'
import CurseforgeDownloadModal from '@/components/ui/install_flow/CurseforgeDownloadModal.vue'
import OptifineImportModal from '@/components/ui/install_flow/OptifineImportModal.vue'
import UnknownPackWarningModal from '@/components/ui/install_flow/UnknownPackWarningModal.vue'
import IconEditorModal from '@/components/ui/instance_settings/icon-editor-modal/index.vue'
import MinecraftAuthErrorModal from '@/components/ui/minecraft-auth-error-modal/MinecraftAuthErrorModal.vue'
import MinecraftRequiredModal from '@/components/ui/minecraft-required-modal/MinecraftRequiredModal.vue'
import MinecraftEditionSelector from '@/components/ui/MinecraftEditionSelector.vue'
import AppSettingsModal from '@/components/ui/modal/AppSettingsModal.vue'
import LaunchLinkConfirmModal from '@/components/ui/modal/LaunchLinkConfirmModal.vue'
import ModpackAlreadyInstalledModal from '@/components/ui/modal/ModpackAlreadyInstalledModal.vue'
import NavButton from '@/components/ui/NavButton.vue'
import OnboardingChecklist from '@/components/ui/onboarding-checklist/index.vue'
import QuickInstanceSwitcher from '@/components/ui/QuickInstanceSwitcher.vue'
import SidebarContent from '@/components/ui/sidebar/SidebarContent.vue'
import SplashScreen from '@/components/ui/SplashScreen.vue'
import SyncInstancesUpdateModal from '@/components/ui/sync-instances-update-modal/index.vue'
import {
	markSyncInstancesUpdateNotificationShown,
	shouldShowSyncInstancesUpdateNotification,
} from '@/components/ui/sync-instances-update-modal/show-notification'
import WindowControls from '@/components/ui/WindowControls.vue'
import InstallWorldModal from '@/components/ui/world/modal/InstallWorldModal.vue'
import { useCheckDisableMouseover } from '@/composables/macCssFix.js'
import { useAppEvent } from '@/composables/use-app-event'
import { useAppSettings } from '@/composables/use-app-settings.ts'
import { useDisplayPreferences } from '@/composables/use-display-preferences'
import { useError } from '@/composables/use-error.js'
import { useIconMotion } from '@/composables/use-icon-motion'
import { useInstanceMetadataRefresh } from '@/composables/use-instance-metadata-refresh'
import { useNavExpanded } from '@/composables/use-nav-expanded'
import { useTheme } from '@/composables/use-theme.ts'
import { config } from '@/config'
import { check_reachable } from '@/helpers/auth.js'
import { setBedrockInstallHandler } from '@/helpers/bedrock-catalog'
import { get_version } from '@/helpers/cache.js'
import {
	parseCurseforgeManualDownload,
	requestCurseforgeManualDownload,
	setCurseforgeManualDownloadHandler,
} from '@/helpers/curseforge'
import { getRightPanelLayout } from '@/helpers/display-preferences'
import { gameSettingsQueryOptions } from '@/helpers/game-options'
import {
	install_create_modpack_instance,
	install_get_modpack_preview,
	install_job_retry,
} from '@/helpers/install'
import { get as getInstance, run } from '@/helpers/instance'
import { maxMemoryQueryOptions } from '@/helpers/jre.js'
import { mergeUrlQuery, parseModrinthLink } from '@/helpers/project-links.ts'
import {
	appSettingsQueryOptions,
	get as getSettings,
	set as setSettings,
} from '@/helpers/settings.ts'
import { debugStartup, traceStartupStep } from '@/helpers/startup-debug'
import { get_opening_command, initialize_state } from '@/helpers/state'
import {
	gameOptionsSyncSourcesQueryOptions,
	globalSyncedOptionsQueryOptions,
	initializedSyncedOptionsQueryOptions,
	syncedServersQueryOptions,
} from '@/helpers/synced-options'
import { syncedPackQueryOptions } from '@/helpers/synced-packs'
import {
	areUpdatesEnabled,
	enqueueUpdateForInstallation,
	getOS,
	getUpdateSize,
	isDev,
	isNetworkMetered,
	setRestartAfterPendingUpdate,
} from '@/helpers/utils.js'
import { setWorldInstallHandler } from '@/helpers/world-install'
import { start_join_server, start_join_singleplayer_world } from '@/helpers/worlds.ts'
import { setLocale } from '@/i18n.config'
import { instanceListQueryOptions } from '@/pages/instance/query-options'
import {
	appUpdateState,
	downloadAvailableAppUpdate,
	getNextAppUpdatePopupTime,
	installAvailableAppUpdate,
	markAppUpdateActionable,
	markAppUpdatePopupShown,
	openAppUpdateChangelog,
	setAppUpdateActions,
} from '@/providers/app-update.ts'
import { createBreadcrumbManager, provideBreadcrumbManager } from '@/providers/breadcrumbs'
import { createContentInstall, provideContentInstall } from '@/providers/content-install'
import {
	provideAppUpdateDownloadProgress,
	subscribeToDownloadProgress,
} from '@/providers/download-progress.ts'
import { setupProviders } from '@/providers/setup'
import { setupAppEventsProvider } from '@/providers/setup/app-events'
import { setupLoadingStateProvider } from '@/providers/setup/loading-state'
import { appMessages } from '@/utils/app-messages'

import { AppNotificationManager } from './providers/app-notifications'
import { AppPopupNotificationManager } from './providers/app-popup-notifications'
import { appSettingsModalOpenSyncedOptionsKey } from './providers/app-settings-modal'

debugStartup('App setup entered')
const appSettings = useAppSettings()
const { navExpanded, toggleNavExpanded } = useNavExpanded()
const leftBarWidth = computed(() => (navExpanded.value ? '14rem' : '4rem'))
const leftBarStyle = computed(() => ({ '--left-bar-width': leftBarWidth.value }))
const appTheme = useTheme()
const router = useRouter()
const route = useRoute()
const isBedrock = computed(
	() => route.path.startsWith('/bedrock') || route.query.edition === 'bedrock',
)
const { channel: appEventChannel, events: appEvents } = setupAppEventsProvider()
useInstanceMetadataRefresh(appEvents)
useIconMotion()
useDisplayPreferences()
const breadcrumbManager = createBreadcrumbManager()
provideBreadcrumbManager(breadcrumbManager)
const canNavigateBack = ref(false)
const canNavigateForward = ref(false)

function updateHistoryNavigationState() {
	const historyState = window.history.state
	canNavigateBack.value = historyState?.back != null
	canNavigateForward.value = historyState?.forward != null
}

updateHistoryNavigationState()

const APP_SIDEBAR_WIDTH = '20.25rem'
const sidebarLayout = computed(() => getRightPanelLayout(route.path))
const sidebarVisible = computed(() => sidebarLayout.value !== 'none')

const notificationManager = new AppNotificationManager(() => ({
	title: formatMessage(messages.curseforgeLimitTitle),
	text: formatMessage(messages.curseforgeLimitText),
}))
provideNotificationManager(notificationManager)
const { handleError, addNotification } = notificationManager

useAppEvent(
	'warning',
	(event) =>
		addNotification({
			title: formatMessage(messages.warning),
			text: event.message,
			type: 'warning',
		}),
	appEvents,
)

const curseforgeDownloadModal = ref(null)
const installWorldModal = ref(null)
const bedrockInstallModal = ref(null)
setBedrockInstallHandler(
	(request) => bedrockInstallModal.value?.show(request) ?? Promise.resolve(null),
)
setWorldInstallHandler((request) => installWorldModal.value?.show(request) ?? Promise.resolve(null))
setCurseforgeManualDownloadHandler(
	(file) => curseforgeDownloadModal.value?.show(file) ?? Promise.resolve(null),
)
// A provider can change distribution permissions between pack preview and installation.
const manualJobs = new Set()
useAppEvent(
	'install_job',
	async (job) => {
		if (job.status !== 'failed' || manualJobs.has(job.job_id)) return
		const manual = parseCurseforgeManualDownload(job.error?.message)
		if (!manual) return
		manualJobs.add(job.job_id)
		try {
			await requestCurseforgeManualDownload(manual)
			await install_job_retry(job.job_id)
		} catch (error) {
			handleError(error)
		} finally {
			manualJobs.delete(job.job_id)
		}
	},
	appEvents,
)

const popupNotificationManager = new AppPopupNotificationManager()
providePopupNotificationManager(popupNotificationManager)
const { addPopupNotification } = popupNotificationManager

const appVersion = getVersion()
const tauriApiClient = new TauriApiClient({
	userAgent: async () => `${productName}/${await appVersion} (${supportEmail})`,
	labrinthBaseUrl: config.labrinthBaseUrl,
	features: [new VerboseLoggingFeature()],
})
provideApiClient(tauriApiClient)
providePageContext({
	hierarchicalSidebarAvailable: ref(true),
	floatingActionBarOffsets: {
		left: leftBarWidth,
		right: computed(() => (sidebarVisible.value ? APP_SIDEBAR_WIDTH : '0px')),
	},
	featureFlags: {
		serverRamAsBytesAlwaysOn: computed(() =>
			appSettings.getFeatureFlag('server_ram_as_bytes_always_on'),
		),
	},
	openExternalUrl: (url) => void openUrl(url),
})
provideModalBehavior({
	noblur: computed(() => !appTheme.advancedRendering),
})

const creationIconEditorModal = ref(null)
const creationGeneratedIcon = ref(null)
const creationIconTarget = ref('creation-flow')

const {
	installationModal,
	openOptifineDownloads,
	pickOptifineInstaller,
	unknownPackWarningModal,
	fetchExistingInstanceNames,
	handleCreate,
	handleBrowseModpacks,
	searchProjects,
	getLoaderManifest,
	setModpackAlreadyInstalledModal,
	handleModpackDuplicateCreateAnyway,
	handleModpackDuplicateGoToInstance,
	onboardingChecklist,
	tags,
} = setupProviders(notificationManager, popupNotificationManager, appEvents, (iconPath) =>
	creationGeneratedIcon.value?.path === iconPath ? creationGeneratedIcon.value.config : null,
)
const { hasCreatedInstance, hasLoggedIntoMinecraft, isReady, showChecklist } = onboardingChecklist
// Getting-started steps show inside the account menu until they're all done.
const gettingStartedPending = computed(
	() =>
		isReady.value &&
		showChecklist.value &&
		(!hasCreatedInstance.value || !hasLoggedIntoMinecraft.value),
)

async function randomizeCreationIcon() {
	const generated = await creationIconEditorModal.value?.randomizeAndSave()
	if (!generated) return null

	creationGeneratedIcon.value = { path: generated.iconPath, config: generated.config }
	return {
		path: generated.iconPath,
		previewUrl: convertFileSrc(generated.iconPath),
	}
}

function customizeCreationIcon() {
	creationIconTarget.value = 'creation-flow'
	creationIconEditorModal.value?.show()
}

function customizeContentInstallIcon() {
	creationIconTarget.value = 'content-install'
	creationIconEditorModal.value?.show()
}

function onCreationIconSaved(iconPath, config) {
	creationGeneratedIcon.value = { path: iconPath, config }
	if (creationIconTarget.value === 'content-install') {
		modInstallModal.value?.setIcon(iconPath, convertFileSrc(iconPath))
		return
	}

	const context = installationModal.value?.ctx
	if (!context) return

	context.instanceIcon.value = null
	context.instanceIconUrl.value = convertFileSrc(iconPath)
	context.instanceIconPath.value = iconPath
}

const offline = ref(!navigator.onLine)
window.addEventListener('offline', () => {
	offline.value = true
})
window.addEventListener('online', () => {
	offline.value = false
})

const os = ref('')
const isDevEnvironment = ref(false)

const stateInitialized = ref(false)
const globalSyncedOptionsQuery = useQuery({
	...globalSyncedOptionsQueryOptions(),
	enabled: computed(() => stateInitialized.value),
})
useQueries({
	queries: computed(() =>
		[
			appSettingsQueryOptions(),
			instanceListQueryOptions(),
			maxMemoryQueryOptions(),
			gameSettingsQueryOptions(),
			initializedSyncedOptionsQueryOptions(),
			gameOptionsSyncSourcesQueryOptions(),
			syncedServersQueryOptions(),
			syncedPackQueryOptions('resourcepack'),
			syncedPackQueryOptions('datapack'),
		].map((options) => ({
			...options,
			enabled: stateInitialized.value && options.enabled !== false,
		})),
	),
})

const isMaximized = ref(false)
const isFullscreen = ref(false)

watch([os, isFullscreen], ([osName, fullscreen]) => {
	document.documentElement.classList.toggle('mac-traffic-lights', osName === 'MacOS' && !fullscreen)
})

const authUnreachableDebug = useDebugLogger('AuthReachableChecker')
const authServerQuery = useQuery({
	queryKey: ['authServerReachability'],
	queryFn: async () => {
		await check_reachable()
		authUnreachableDebug('Auth servers are reachable')
		return true
	},
	refetchInterval: 5 * 60 * 1000, // 5 minutes
	retry: false,
	refetchOnWindowFocus: false,
})

const authUnreachable = computed(() => {
	if (authServerQuery.isError.value && !authServerQuery.isLoading.value) {
		console.warn('Failed to reach auth servers', authServerQuery.error.value)
		return true
	}
	return false
})

let unlistenEditMenu

function handleEditMenuAction(action) {
	const event = new CustomEvent(`edit-menu:${action}`, { cancelable: true })
	if (document.dispatchEvent(event)) document.execCommand(action)
}

onMounted(async () => {
	try {
		const listeners = await Promise.all([
			listen('edit-menu://undo', () => handleEditMenuAction('undo')),
			listen('edit-menu://redo', () => handleEditMenuAction('redo')),
		])
		unlistenEditMenu = () => listeners.forEach((unlisten) => unlisten())
	} catch (error) {
		handleError(error)
	}

	await useCheckDisableMouseover()
	document.querySelector('body').addEventListener('click', handleClick)
	document.querySelector('body').addEventListener('auxclick', handleAuxClick)
	document.querySelector('body').addEventListener('contextmenu', handleContextMenu)

	checkUpdates()
})

onUnmounted(async () => {
	document.querySelector('body').removeEventListener('click', handleClick)
	document.querySelector('body').removeEventListener('auxclick', handleAuxClick)
	document.querySelector('body').removeEventListener('contextmenu', handleContextMenu)
	unlistenEditMenu?.()
	clearDelayedUpdatePopup()

	await unlistenUpdateDownload?.()
})

const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()

const messages = defineMessages({
	curseforgeLimitTitle: {
		id: 'app.curseforge.rate-limit.title',
		defaultMessage: 'CurseForge is temporarily busy',
	},
	curseforgeLimitText: {
		id: 'app.curseforge.rate-limit.text',
		defaultMessage: 'The shared request limit has been reached. Wait a moment, then try again.',
	},

	syncUpdateTitle: {
		id: 'app.sync-instances-update.notification.title',
		defaultMessage: 'Sync your instances',
	},
	syncUpdateDescription: {
		id: 'app.sync-instances-update.notification.description',
		defaultMessage:
			'Keep game settings, servers, resource packs, and more in sync across your instances.',
	},
	syncUpdateView: {
		id: 'app.sync-instances-update.notification.view-update',
		defaultMessage: 'View update',
	},
	syncUpdateDismiss: {
		id: 'app.sync-instances-update.notification.dismiss',
		defaultMessage: 'Dismiss',
	},
	warning: { id: 'app.notification.warning', defaultMessage: 'Warning' },
	goBack: { id: 'app.navigation.go-back', defaultMessage: 'Go back' },
	goForward: { id: 'app.navigation.go-forward', defaultMessage: 'Go forward' },
	nextImage: { id: 'app.navigation.next-image', defaultMessage: 'Next image' },
	updateDownloadMissingVersion: {
		id: 'app.update.download-error.missing-version',
		defaultMessage: 'Failed to download update: no version available',
	},
	updateInstalledToastTitle: {
		id: 'app.update.complete-toast.title',
		defaultMessage: 'Version {version} was successfully installed!',
	},
	updateInstalledToastText: {
		id: 'app.update.complete-toast.text',
		defaultMessage: 'Click here to view the changelog.',
	},
	authUnreachableHeader: {
		id: 'app.auth-servers.unreachable.header',
		defaultMessage: 'Cannot reach authentication servers',
	},
	authUnreachableBody: {
		id: 'app.auth-servers.unreachable.body',
		defaultMessage:
			'Minecraft authentication servers may be down right now. Check your internet connection and try again later.',
	},
	home: {
		id: 'app.nav.home',
		defaultMessage: 'Home',
	},
	collapseSidebar: {
		id: 'app.nav.collapse-sidebar',
		defaultMessage: 'Collapse sidebar',
	},
	expandSidebar: {
		id: 'app.nav.expand-sidebar',
		defaultMessage: 'Expand sidebar',
	},
	screenshots: {
		id: 'app.nav.screenshots',
		defaultMessage: 'Screenshots',
	},
	createNewInstance: {
		id: 'app.nav.create-new-instance',
		defaultMessage: 'Create new instance',
	},
	restarting: {
		id: 'app.restarting',
		defaultMessage: 'Restarting...',
	},
})

async function setupApp() {
	tags.initialize()
	await traceStartupStep('Initialize onboarding checklist', () => onboardingChecklist.initialize())

	const {
		native_decorations,
		theme,
		locale,
		hide_nametag_skins_page,
		advanced_rendering,
		show_files_tab_in_instances,
		show_worlds_tab_in_instances,
		show_screenshots_tab_in_instances,
		show_skin_selector_in_sidebar,
		developer_mode,
		feature_flags,
		pending_update_toast_for_version,
	} = await traceStartupStep('Read startup settings', getSettings)

	// Initialize locale from saved settings
	if (locale) {
		await traceStartupStep('Apply startup locale', () => setLocale(locale))
	}

	Object.assign(appSettings.featureFlags, feature_flags)
	isMaximized.value = await traceStartupStep('Read window maximized state', () =>
		getCurrentWindow().isMaximized(),
	)
	isFullscreen.value = await traceStartupStep('Read window fullscreen state', () =>
		getCurrentWindow().isFullscreen(),
	)
	os.value = await traceStartupStep('Read operating system', getOS)
	const dev = await traceStartupStep('Read development mode', isDev)
	isDevEnvironment.value = dev
	const version = await traceStartupStep('Read app version', getVersion)
	appSettings.nativeDecorations = native_decorations
	if (os.value !== 'MacOS') {
		await traceStartupStep('Apply window decorations', () =>
			getCurrentWindow().setDecorations(native_decorations),
		)
	}

	appTheme.preferred = theme
	appTheme.advancedRendering = advanced_rendering
	appSettings.hideNametagSkinsPage = hide_nametag_skins_page
	appSettings.showFilesTabInInstances = show_files_tab_in_instances
	appSettings.showWorldsTabInInstances = show_worlds_tab_in_instances
	appSettings.showScreenshotsTabInInstances = show_screenshots_tab_in_instances
	appSettings.showSkinSelectorInSidebar = show_skin_selector_in_sidebar
	appSettings.devMode = developer_mode
	stateInitialized.value = true
	debugStartup('App state initialized')
	await traceStartupStep('Render initialized app', nextTick)
	const isSyncUpdateVersion = version.startsWith('0.20.')
	if (isSyncUpdateVersion && pending_update_toast_for_version !== version) {
		markSyncInstancesUpdateNotificationShown()
	}
	if (
		appSettings.getFeatureFlag('show_sync_instances_update_modal') ||
		(isSyncUpdateVersion &&
			pending_update_toast_for_version === version &&
			(
				await traceStartupStep('Load instances for update notification', () =>
					queryClient.fetchQuery(instanceListQueryOptions()),
				)
			).length > 0)
	) {
		showSyncInstancesUpdateNotification()
	}

	await traceStartupStep('Register window resize listener', () =>
		getCurrentWindow().onResized(async () => {
			isMaximized.value = await getCurrentWindow().isMaximized()
			isFullscreen.value = await getCurrentWindow().isFullscreen()
		}),
	)

	const osType = await traceStartupStep('Read operating system type', async () => type())
	if (osType === 'macos') {
		document.getElementsByTagName('html')[0].classList.add('mac')
	} else {
		document.getElementsByTagName('html')[0].classList.add('windows')
	}

	traceStartupStep('Read opening command', get_opening_command).then(handleCommand)

	if (pending_update_toast_for_version !== null) {
		const settings = await traceStartupStep(
			'Read settings to clear update notification',
			getSettings,
		)
		settings.pending_update_toast_for_version = null
		await traceStartupStep('Clear update notification', () => setSettings(settings))
	}
}

const stateFailed = ref(false)
traceStartupStep('Initialize backend state', () => initialize_state(appEventChannel))
	.then(() => {
		traceStartupStep('Initialize frontend state', setupApp).catch((err) => {
			stateFailed.value = true
			console.error(err)
			error.showError(err, null, false, 'state_init')
		})
	})
	.catch((err) => {
		stateFailed.value = true
		console.error('Failed to initialize app', err)
		error.showError(err, null, false, 'state_init')
	})

const handleClose = async () => {
	await saveWindowState(StateFlags.ALL)
	await getCurrentWindow().close()
}

const loading = setupLoadingStateProvider(() => ({
	stateInitialized: stateInitialized.value,
	stateFailed: stateFailed.value,
	initialStatePending: !!initialLoadToken,
	navigationPending: !!routerToken,
	routeSuspensePending: !!suspenseToken,
	route: route.path,
}))
loading.setEnabled(false)
let initialLoadToken = loading.begin('Initial app state')
let routerToken = null
let suspenseToken = null

let suspensePending = false

const sidebarOverlayScrollbarsOptions = Object.freeze({
	overflow: {
		x: 'hidden',
		y: 'scroll',
	},
})

router.beforeEach((to, from) => {
	debugStartup('Route navigation started', { to: to.path, from: from.path })
	suspensePending = false
	if (routerToken) loading.end(routerToken)
	routerToken = loading.begin(`Route navigation: ${to.path}`)
})
router.afterEach((to, from, failure) => {
	debugStartup('Route navigation settled', { to: to.path, failed: !!failure })
	updateHistoryNavigationState()
	setTimeout(() => {
		debugStartup('Route loading release check', {
			route: to.path,
			suspensePending,
			stateInitialized: stateInitialized.value,
		})
		if (!suspensePending && stateInitialized.value) {
			if (initialLoadToken) {
				loading.end(initialLoadToken)
				initialLoadToken = null
			}
			if (routerToken) {
				loading.end(routerToken)
				routerToken = null
			}
		}
	}, 100)
})

function onSuspensePending() {
	debugStartup('Route Suspense pending', { route: route.path })
	suspensePending = true
	if (suspenseToken) loading.end(suspenseToken)
	suspenseToken = loading.begin(`Route Suspense: ${route.path}`)
}

function onSuspenseResolve() {
	debugStartup('Route Suspense resolved', { route: route.path })
	if (suspenseToken) {
		loading.end(suspenseToken)
		suspenseToken = null
	}
	if (routerToken) {
		loading.end(routerToken)
		routerToken = null
	}
}

const queryClient = useQueryClient()

watch(stateInitialized, (ready) => {
	debugStartup('State readiness changed', { ready })
	if (ready) {
		if (initialLoadToken) {
			loading.end(initialLoadToken)
			initialLoadToken = null
		}
		if (routerToken) {
			loading.end(routerToken)
			routerToken = null
		}
	}
})

const error = useError()
const errorModal = ref()
const minecraftAuthErrorModal = ref()
const minecraftRequiredModal = ref()

const contentInstall = createContentInstall({ router, handleError, appEvents })
provideContentInstall(contentInstall)
const {
	instances: contentInstallInstances,
	compatibleLoaders: contentInstallLoaders,
	gameVersions: contentInstallGameVersions,
	loading: contentInstallLoading,
	defaultTab: contentInstallDefaultTab,
	preferredLoader: contentInstallPreferredLoader,
	preferredGameVersion: contentInstallPreferredGameVersion,
	releaseGameVersions: contentInstallReleaseGameVersions,
	projectInfo: contentInstallProjectInfo,
	handleInstallToInstance,
	handleCreateAndInstall,
	prepareNewInstance,
	handleNavigate: handleContentInstallNavigate,
	handleCancel: handleContentInstallCancel,
	setContentInstallModal,
	setModpackAlreadyInstalledModal: setContentInstallModpackAlreadyInstalledModal,
	handleModpackDuplicateCreateAnyway: handleContentInstallModpackDuplicateCreateAnyway,
	handleModpackDuplicateGoToInstance: handleContentInstallModpackDuplicateGoToInstance,
	setIncompatibilityWarningModal: setContentIncompatibilityWarningModal,
	incompatibilityWarningVersions: contentInstallIncompatibilityWarningVersions,
	incompatibilityWarningCurrentGameVersion: contentInstallIncompatibilityWarningCurrentGameVersion,
	incompatibilityWarningCurrentLoader: contentInstallIncompatibilityWarningCurrentLoader,
	incompatibilityWarningProjectType: contentInstallIncompatibilityWarningProjectType,
	incompatibilityWarningProjectIconUrl: contentInstallIncompatibilityWarningProjectIconUrl,
	incompatibilityWarningProjectName: contentInstallIncompatibilityWarningProjectName,
	incompatibilityWarningMessage: contentInstallIncompatibilityWarningMessage,
	incompatibilityWarningInstalling: contentInstallIncompatibilityWarningInstalling,
	handleIncompatibilityWarningInstall: handleContentInstallIncompatibilityWarningInstall,
	handleIncompatibilityWarningCancel: handleContentInstallIncompatibilityWarningCancel,
} = contentInstall

async function prepareCreationProjectInstall(projectId, projectType) {
	if (projectType === 'modpack') {
		await contentInstall.install(
			projectId,
			null,
			null,
			'CreationModalProject',
			undefined,
			(instanceId) => void router.push(`/instance/${encodeURIComponent(instanceId)}`),
		)
		return null
	}

	await prepareNewInstance(projectId)
	const info = contentInstallProjectInfo.value
	if (!info) throw new Error(`Project information is unavailable: '${projectId}'`)

	return {
		projectId,
		title: info.title,
		iconUrl: info.iconUrl,
		link: info.link,
		owner: info.owner,
		compatibleLoaders: [...contentInstallLoaders.value],
		gameVersions: [...contentInstallGameVersions.value],
		releaseGameVersions: new Set(contentInstallReleaseGameVersions.value),
	}
}

const modInstallModal = ref()
const launchLinkConfirmModal = ref()
const modpackAlreadyInstalledModal = ref()
const contentInstallModpackAlreadyInstalledModal = ref()
const incompatibilityWarningModal = ref()

const appSettingsModal = ref()
const syncInstancesUpdateModal = ref()
let syncInstancesUpdateNotificationId = null

function showSyncInstancesUpdateNotification() {
	if (
		popupNotificationManager
			.getNotifications()
			.some((notification) => notification.id === syncInstancesUpdateNotificationId)
	) {
		return
	}

	if (!shouldShowSyncInstancesUpdateNotification()) return

	const notification = addPopupNotification({
		contentType: 'standard',
		title: formatMessage(messages.syncUpdateTitle),
		text: formatMessage(messages.syncUpdateDescription),
		type: 'info',
		hideIcon: true,
		autoCloseMs: null,
		buttons: [
			{
				label: formatMessage(messages.syncUpdateDismiss),
				color: 'standard',
				action: () => popupNotificationManager.removeNotification(notification.id),
			},
			{
				label: formatMessage(messages.syncUpdateView),
				color: 'brand',
				action: () => syncInstancesUpdateModal.value?.show(),
			},
		],
	})
	syncInstancesUpdateNotificationId = notification.id
}

provide(appSettingsModalOpenSyncedOptionsKey, () => appSettingsModal.value?.showSyncedOptions())

watch(
	() => appSettings.getFeatureFlag('show_sync_instances_update_modal'),
	(enabled) => {
		if (enabled && stateInitialized.value) {
			showSyncInstancesUpdateNotification()
		}
	},
)

watch(incompatibilityWarningModal, (modal) => {
	if (modal) {
		setContentIncompatibilityWarningModal(modal)
	}
})

onMounted(() => {
	invoke('show_window')

	error.setErrorModal(errorModal.value)
	error.setMinecraftAuthErrorModal(minecraftAuthErrorModal.value)
	error.setMinecraftRequiredModal(minecraftRequiredModal.value)

	setContentIncompatibilityWarningModal(incompatibilityWarningModal.value)
	setContentInstallModal(modInstallModal.value)
	setContentInstallModpackAlreadyInstalledModal(contentInstallModpackAlreadyInstalledModal.value)
	setModpackAlreadyInstalledModal(modpackAlreadyInstalledModal.value)
})

const accounts = ref(null)
provide('accountsCard', accounts)

useAppEvent('command', handleCommand, appEvents)

async function handleCommand(e) {
	if (!e) return

	if (e.event === 'RunMRPack') {
		// Keep the existing command event compatible for all supported pack files.
		if (/\.(orbpack|mrpack|zip)$/i.test(e.path)) {
			const location = { type: 'fromFile', path: e.path }
			const preview = await install_get_modpack_preview(location).catch(handleError)
			if (!preview) return
			if (preview?.unknownFile || preview?.externalFilesInModpack.length > 0) {
				const splitPath = e.path.split(/[\\/]/)
				const fileName = splitPath ? splitPath[splitPath.length - 1] : e.path
				unknownPackWarningModal.value?.show(
					() => install_create_modpack_instance(location).then(() => undefined),
					fileName,
					preview.externalFilesInModpack,
				)
			} else {
				await install_create_modpack_instance(location).catch(handleError)
			}
		}
	} else if (e.event === 'LaunchInstance') {
		const instance = await getInstance(e.id).catch(handleError)
		if (!instance || instance.quarantined) return

		// Any website can open an orbiont:// link: never start the game or
		// join a server from one without asking.
		const target = e.server
			? { kind: 'server', name: e.server }
			: e.singleplayer_world
				? { kind: 'world', name: e.singleplayer_world }
				: null
		const confirmed = await launchLinkConfirmModal.value?.ask(instance.name, target)
		if (!confirmed) return

		if (e.server) {
			await start_join_server(e.id, e.server).catch(handleError)
		} else if (e.singleplayer_world) {
			await start_join_singleplayer_world(e.id, e.singleplayer_world).catch(handleError)
		} else {
			await run(e.id).catch(handleError)
		}
	} else if (e.event === 'InstallVersion') {
		const version = await get_version(e.id, 'must_revalidate').catch(handleError)
		if (version) {
			await contentInstall
				.install(version.project_id, version.id, null, 'URLConfirmModal', undefined, undefined, {
					showProjectInfo: true,
				})
				.catch(handleError)
		}
	} else {
		await contentInstall
			.install(e.id, null, null, 'URLConfirmModal', undefined, undefined, { showProjectInfo: true })
			.catch(handleError)
	}
}

const appUpdateDownload = {
	progress: appUpdateState.progress,
	version: ref(),
}
let unlistenUpdateDownload

const {
	metered,
	finishedDownloading,
	downloading,
	restarting,
	availableUpdate,
	updateSize,
	updatesEnabled,
} = appUpdateState
let delayedUpdatePopupTimeout = null

const updatePopupMessages = defineMessages({
	updateAvailable: {
		id: 'app.update-popup.title',
		defaultMessage: 'Update available',
	},
	downloadComplete: {
		id: 'app.update-popup.download-complete',
		defaultMessage: 'Download complete',
	},
	meteredBody: {
		id: 'app.update-popup.body.metered',
		defaultMessage: `{productName} v{version} is available now! Since you're on a metered network, we didn't automatically download it.`,
	},
	downloadedBody: {
		id: 'app.update-popup.body.download-complete',
		defaultMessage: `{productName} v{version} has finished downloading. Reload to update now, or automatically when you close {productName}.`,
	},
	linuxBody: {
		id: 'app.update-popup.body.linux',
		defaultMessage:
			'{productName} v{version} is available. Use your package manager to update for the latest features and fixes!',
	},
	reload: {
		id: 'app.update-popup.reload',
		defaultMessage: 'Reload to update',
	},
	download: {
		id: 'app.update-popup.download',
		defaultMessage: 'Download ({size})',
	},
	changelog: {
		id: 'app.update-popup.changelog',
		defaultMessage: 'Changelog',
	},
})

function clearDelayedUpdatePopup() {
	if (delayedUpdatePopupTimeout !== null) {
		clearTimeout(delayedUpdatePopupTimeout)
		delayedUpdatePopupTimeout = null
	}
}

function getCurrentUpdatePromptStage() {
	return finishedDownloading.value ? 'downloaded' : 'available'
}

function scheduleDelayedUpdatePopup() {
	clearDelayedUpdatePopup()

	const version = availableUpdate.value?.version
	if (!version) {
		return
	}

	const nextPopupTime = getNextAppUpdatePopupTime(version, getCurrentUpdatePromptStage())
	if (nextPopupTime === null) {
		return
	}

	const delay = nextPopupTime - Date.now()
	if (delay <= 0) {
		showDelayedUpdatePopup()
		return
	}

	delayedUpdatePopupTimeout = setTimeout(showDelayedUpdatePopup, Math.min(delay, 2_147_483_647))
}

function showDelayedUpdatePopup() {
	const update = availableUpdate.value
	if (!update) {
		return
	}

	const stage = getCurrentUpdatePromptStage()
	const nextPopupTime = getNextAppUpdatePopupTime(update.version, stage)
	if (nextPopupTime === null) {
		return
	}

	if (Date.now() < nextPopupTime) {
		scheduleDelayedUpdatePopup()
		return
	}

	if (metered.value && !finishedDownloading.value) {
		addPopupNotification({
			contentType: 'standard',
			title: formatMessage(updatePopupMessages.updateAvailable),
			text: formatMessage(updatePopupMessages.meteredBody, { version: update.version }),
			type: 'info',
			autoCloseMs: null,
			buttons: [
				{
					label: formatMessage(updatePopupMessages.download, {
						size: formatBytes(updateSize.value ?? 0),
					}),
					action: () => downloadAvailableAppUpdate(),
					color: 'brand',
				},
				{
					label: formatMessage(updatePopupMessages.changelog),
					action: () => openAppUpdateChangelog(),
					keepOpen: true,
				},
			],
		})
	} else if (finishedDownloading.value) {
		addPopupNotification({
			contentType: 'standard',
			title: formatMessage(updatePopupMessages.downloadComplete),
			text: formatMessage(updatePopupMessages.downloadedBody, {
				version: update.version,
			}),
			type: 'success',
			autoCloseMs: null,
			buttons: [
				{
					label: formatMessage(updatePopupMessages.reload),
					action: () => installAvailableAppUpdate(),
					color: 'brand',
				},
				{
					label: formatMessage(updatePopupMessages.changelog),
					action: () => openAppUpdateChangelog(),
					keepOpen: true,
				},
			],
		})
	} else {
		scheduleDelayedUpdatePopup()
		return
	}

	markAppUpdatePopupShown(update.version, stage)
}

async function checkUpdates() {
	if (!(await areUpdatesEnabled())) {
		console.log('Skipping update check as updates are disabled in this build or environment')
		updatesEnabled.value = false

		if (os.value === 'Linux' && !isDevEnvironment.value) {
			checkLinuxUpdates()
			setInterval(checkLinuxUpdates, 5 * 60 * 1000)
		}
		return
	}

	async function performCheck() {
		const update = await invoke('plugin:updater|check')
		if (!update) {
			console.log('No update available')
			return
		}

		const isExistingUpdate = update.version === availableUpdate.value?.version

		if (isExistingUpdate) {
			console.log('Update is already known')
			scheduleDelayedUpdatePopup()
			return
		}

		appUpdateDownload.progress.value = 0
		finishedDownloading.value = false
		downloading.value = false
		updateSize.value = null
		availableUpdate.value = update

		console.log(`Update ${update.version} is available.`)

		metered.value = await isNetworkMetered()
		if (!metered.value) {
			console.log('Starting download of update')
			downloadUpdate(update)
		} else {
			console.log(`Metered connection detected, not auto-downloading update.`)
			markAppUpdateActionable(update.version)
			scheduleDelayedUpdatePopup()
		}

		getUpdateSize(update.rid).then((size) => (updateSize.value = size))
	}

	await performCheck()
	setTimeout(
		() => {
			checkUpdates()
		},
		5 /* min */ * 60 /* sec */ * 1000 /* ms */,
	)
}

async function checkLinuxUpdates() {
	try {
		const [response, currentVersion] = await Promise.all([fetch(config.updatesUrl), getVersion()])
		const updates = await response.json()
		const latestVersion = updates?.version

		if (latestVersion && latestVersion !== currentVersion) {
			markAppUpdateActionable(latestVersion)
			const nextPopupTime = getNextAppUpdatePopupTime(latestVersion)
			if (nextPopupTime !== null && Date.now() >= nextPopupTime) {
				addPopupNotification({
					contentType: 'standard',
					title: formatMessage(updatePopupMessages.updateAvailable),
					text: formatMessage(updatePopupMessages.linuxBody, { version: latestVersion }),
					type: 'info',
					autoCloseMs: null,
				})
				markAppUpdatePopupShown(latestVersion)
			}
		}
	} catch (e) {
		console.error('Failed to check for updates:', e)
	}
}

async function downloadAvailableUpdate() {
	return downloadUpdate(availableUpdate.value)
}

async function downloadUpdate(versionToDownload) {
	if (!versionToDownload) {
		handleError(formatMessage(messages.updateDownloadMissingVersion))
		return
	}

	if (downloading.value || appUpdateDownload.progress.value !== 0) {
		console.error(`Update ${versionToDownload.version} already downloading`)
		return
	}

	console.log(`Downloading update ${versionToDownload.version}`)
	downloading.value = true

	try {
		enqueueUpdateForInstallation(versionToDownload.rid)
			.then(() => {
				downloading.value = false
				finishedDownloading.value = true
				unlistenUpdateDownload?.()
				unlistenUpdateDownload = null
				console.log('Finished downloading!')
				markAppUpdateActionable(versionToDownload.version, 'downloaded')
				scheduleDelayedUpdatePopup()
			})
			.catch((e) => {
				downloading.value = false
				appUpdateDownload.progress.value = 0
				handleError(e)
			})
		unlistenUpdateDownload = await subscribeToDownloadProgress(
			appEvents,
			appUpdateDownload,
			versionToDownload.version,
		)
	} catch (e) {
		downloading.value = false
		appUpdateDownload.progress.value = 0
		handleError(e)
	}
}

async function installUpdate() {
	restarting.value = true

	try {
		await setRestartAfterPendingUpdate(true)
	} catch (e) {
		restarting.value = false
		handleError(e)
		return
	}
	setTimeout(async () => {
		await handleClose()
	}, 250)
}

setAppUpdateActions({
	download: downloadAvailableUpdate,
	install: installUpdate,
	changelog: () => openUrl(changelogUrl),
})

async function openModrinthProjectLinkInApp(parsed) {
	const { slug, pathSuffix, url } = parsed
	const loadToken = loading.begin()
	try {
		const { id } = await tauriApiClient.labrinth.projects_v2.check(slug)
		const query = mergeUrlQuery(route.query, url)
		await router.push({
			path: `/project/${id}${pathSuffix}`,
			query,
			hash: url.hash || undefined,
		})
	} catch (err) {
		if (err instanceof ApiError && err.statusCode === 404) {
			openUrl(url.href)
		} else {
			handleError(err)
		}
	} finally {
		loading.end(loadToken)
	}
}

function handleClick(e) {
	let target = e.target
	while (target != null) {
		if (target.matches('a')) {
			if (
				target.href &&
				['http://', 'https://', 'mailto:', 'tel:'].some((v) => target.href.startsWith(v)) &&
				!target.classList.contains('router-link-active') &&
				!target.href.startsWith('http://localhost') &&
				!target.href.startsWith('https://tauri.localhost') &&
				!target.href.startsWith('http://tauri.localhost')
			) {
				const parsed = parseModrinthLink(target.href)
				if (target.target !== '_blank' && parsed) {
					void openModrinthProjectLinkInApp(parsed)
				} else {
					openUrl(target.href)
				}
			}
			e.preventDefault()
			break
		}
		target = target.parentElement
	}
}

function handleAuxClick(e) {
	// disables middle click -> new tab
	if (e.button === 1) {
		e.preventDefault()
		// instead do a left click
		const event = new MouseEvent('click', {
			view: window,
			bubbles: true,
			cancelable: true,
		})
		e.target.dispatchEvent(event)
	}
}

function handleContextMenu(event) {
	const target = event.target
	if (target instanceof Element) {
		if (target.closest('img, textarea, [contenteditable="true"]')) return
		const input = target.closest('input')
		if (
			input &&
			!['button', 'checkbox', 'radio', 'submit', 'reset', 'file', 'range'].includes(input.type)
		) {
			return
		}
	}

	const selection = window.getSelection()
	if (
		target instanceof Node &&
		selection &&
		!selection.isCollapsed &&
		selection.containsNode(target, true)
	) {
		return
	}

	event.preventDefault()
}

provideAppUpdateDownloadProgress(appUpdateDownload)
</script>

<template>
	<TooltipDirective />
	<SplashScreen v-if="!stateFailed" ref="splashScreen" data-tauri-drag-region />
	<div id="teleports"></div>
	<div
		v-if="stateInitialized"
		class="app-grid-layout relative"
		:style="leftBarStyle"
		:class="{ 'disable-advanced-rendering': !appTheme.advancedRendering }"
	>
		<Transition name="fade">
			<div
				v-if="restarting"
				data-tauri-drag-region
				class="inset-0 fixed bg-black/80 backdrop-blur z-[200] flex items-center justify-center"
			>
				<span
					data-tauri-drag-region
					class="flex items-center gap-4 text-contrast font-semibold text-xl select-none cursor-default"
				>
					<RefreshCwIcon data-tauri-drag-region class="animate-spin w-6 h-6" />
					{{ formatMessage(messages.restarting) }}
				</span>
			</div>
		</Transition>
		<AppSettingsModal ref="appSettingsModal" />
		<SyncInstancesUpdateModal ref="syncInstancesUpdateModal" />
		<CreationFlowModal
			ref="installationModal"
			show-snapshot-toggle
			:fetch-existing-instance-names="fetchExistingInstanceNames"
			:search-projects="searchProjects"
			:prepare-project-install="prepareCreationProjectInstall"
			:create-project-install="handleCreateAndInstall"
			:get-loader-manifest="getLoaderManifest"
			:pick-optifine-installer="pickOptifineInstaller"
			:open-optifine-downloads="openOptifineDownloads"
			:randomize-instance-icon="randomizeCreationIcon"
			:customize-instance-icon="customizeCreationIcon"
			@create="handleCreate"
			@browse-modpacks="handleBrowseModpacks"
		/>
		<IconEditorModal
			ref="creationIconEditorModal"
			:config="creationGeneratedIcon?.config"
			@saved="onCreationIconSaved"
		/>
		<UnknownPackWarningModal ref="unknownPackWarningModal" />
		<OptifineImportModal ref="optifineImportModal" />
		<CurseforgeDownloadModal ref="curseforgeDownloadModal" />
		<BedrockInstallModal ref="bedrockInstallModal" />
		<InstallWorldModal
			ref="installWorldModal"
			@navigate="(id) => router.push(`/instance/${encodeURIComponent(id)}/worlds`)"
		/>
		<div
			class="app-grid-navbar bg-bg-raised flex flex-col p-[0.5rem] pt-0 gap-[0.25rem] w-[--left-bar-width]"
			:class="{ 'app-grid-navbar--expanded': navExpanded }"
		>
			<MinecraftEditionSelector :expanded="navExpanded" class="mb-2 mt-2 shrink-0" />
			<NavButton
				:label="formatMessage(messages.home)"
				:to="isBedrock ? '/bedrock' : '/'"
				:is-primary="(route) => route.path === '/' || route.path === '/bedrock'"
				:is-subpage="
					() =>
						(route.path.startsWith('/browse') || route.path.startsWith('/project')) && route.query.i
				"
			>
				<AnimatedIcon name="play" class="ml-0.5" />
			</NavButton>
			<NavButton
				:label="formatMessage(commonMessages.discoverContentLabel)"
				:to="isBedrock ? '/browse/mod?edition=bedrock&src=curseforge' : '/browse/modpack'"
				:is-primary="() => route.path.startsWith('/browse') && !route.query.i && !route.query.sid"
				:is-subpage="
					(route) => route.path.startsWith('/project') && !route.query.i && !route.query.sid
				"
			>
				<AnimatedIcon name="compass" />
			</NavButton>
			<NavButton
				v-if="!isBedrock && appSettings.showSkinSelectorInSidebar"
				:label="formatMessage(appMessages.skinSelectorLabel)"
				to="/skins"
			>
				<AnimatedIcon name="shirt" />
			</NavButton>
			<NavButton
				v-if="isBedrock || globalSyncedOptionsQuery.data.value?.screenshots"
				:label="formatMessage(messages.screenshots)"
				:to="isBedrock ? '/screenshots?edition=bedrock' : '/screenshots'"
			>
				<AnimatedIcon name="image" />
			</NavButton>
			<suspense v-if="!isBedrock">
				<QuickInstanceSwitcher>
					<NavButton
						:label="formatMessage(messages.createNewInstance)"
						:to="() => installationModal?.show()"
						:disabled="offline"
					>
						<AnimatedIcon name="add" />
					</NavButton>
				</QuickInstanceSwitcher>
			</suspense>
			<div v-if="isBedrock" class="min-h-0 flex-1" />
			<NavButton
				:label="formatMessage(commonMessages.settingsLabel)"
				:to="() => appSettingsModal?.show()"
			>
				<AnimatedIcon name="settings" />
			</NavButton>
		</div>
		<div data-tauri-drag-region class="app-grid-statusbar bg-bg-raised h-[--top-bar-height] flex">
			<div data-tauri-drag-region class="flex min-w-0 flex-1 items-center overflow-hidden p-2">
				<!-- The logo spans the nav column, so the navigation controls line up with
				     the content area in both the expanded and collapsed sidebar. -->
				<div
					data-tauri-drag-region
					class="flex shrink-0 items-center pr-2 transition-[min-width] duration-200"
					:style="{ minWidth: 'calc(var(--left-bar-width) - 0.5rem)' }"
				>
					<AppLogo class="pointer-events-none" />
				</div>
				<div data-tauri-drag-region class="flex shrink-0 items-center gap-2">
					<IconButton
						v-tooltip="
							formatMessage(navExpanded ? messages.collapseSidebar : messages.expandSidebar)
						"
						type="outlined"
						:label="formatMessage(navExpanded ? messages.collapseSidebar : messages.expandSidebar)"
						class="!h-7 !min-w-7 !w-7 !border !border-surface-4 !p-0 !opacity-100"
						@click="toggleNavExpanded"
					>
						<PanelLeftIcon />
					</IconButton>
					<IconButton
						type="outlined"
						:label="formatMessage(messages.goBack)"
						class="!h-7 !min-w-7 !w-7 !border !border-surface-4 !p-0 !opacity-100"
						:disabled="!canNavigateBack"
						@click="router.back()"
					>
						<ChevronLeftIcon
							class="!size-4 !text-primary"
							:class="{ 'opacity-20': !canNavigateBack }"
						/>
					</IconButton>
					<IconButton
						type="outlined"
						:label="formatMessage(messages.goForward)"
						class="!h-7 !min-w-7 !w-7 !border !border-surface-4 !p-0 !opacity-100"
						:disabled="!canNavigateForward"
						@click="router.forward()"
					>
						<ChevronRightIcon
							class="!size-4 !text-primary"
							:class="{ 'opacity-20': !canNavigateForward }"
						/>
					</IconButton>
				</div>
				<Breadcrumbs />
			</div>
			<section data-tauri-drag-region class="flex shrink-0 ml-auto items-center">
				<div class="flex mr-3">
					<Suspense>
						<AppActionBar />
					</Suspense>
				</div>
				<Suspense>
					<AccountsCard ref="accounts" class="mr-3">
						<template v-if="gettingStartedPending" #extra="{ hide }">
							<OnboardingChecklist
								@done="hide()"
								@create-instance="installationModal?.show()"
								@login-minecraft="accounts?.login()"
							/>
						</template>
					</AccountsCard>
				</Suspense>
				<WindowControls />
			</section>
		</div>
	</div>
	<div
		v-if="stateInitialized"
		class="app-contents"
		:style="leftBarStyle"
		:class="{
			'sidebar-enabled': sidebarVisible,
			'disable-advanced-rendering': !appTheme.advancedRendering,
		}"
	>
		<div class="app-viewport flex-grow router-view">
			<div
				class="loading-indicator-container h-8 fixed z-50 pointer-events-none"
				:style="{
					top: 'calc(var(--top-bar-height))',
					left: 'calc(var(--left-bar-width))',
					width: 'calc(100% - var(--left-bar-width) - var(--right-bar-width))',
				}"
			>
				<LoadingBar position="absolute" />
			</div>
			<div
				v-if="appSettings.featureFlags.page_path"
				class="absolute bottom-0 left-0 m-2 bg-tooltip-bg text-tooltip-text font-semibold rounded-full px-2 py-1 text-xs z-50"
			>
				{{ route.fullPath }}
			</div>
			<div
				id="background-teleport-target"
				class="absolute h-full -z-10 rounded-tl-[--radius-xl] overflow-hidden"
				:style="{
					width: 'calc(100% - var(--right-bar-width))',
				}"
			></div>
			<Admonition
				v-if="authUnreachable"
				type="warning"
				:header="formatMessage(messages.authUnreachableHeader)"
				class="m-6 mb-0"
			>
				{{ formatMessage(messages.authUnreachableBody) }}
			</Admonition>
			<RouterView v-slot="{ Component }">
				<template v-if="Component">
					<Suspense @pending="onSuspensePending" @resolve="onSuspenseResolve">
						<component :is="Component" :key="isBedrock ? 'bedrock' : 'java'"></component>
					</Suspense>
				</template>
			</RouterView>
		</div>
		<div
			id="app-right-panel"
			class="app-sidebar mt-px shrink-0 flex flex-col border-0 border-l-[1px] border-[--brand-gradient-border] border-solid"
		>
			<div
				v-overlay-scrollbars="sidebarOverlayScrollbarsOptions"
				class="app-sidebar-scrollable flex-grow shrink relative"
				data-overlayscrollbars-initialize
			>
				<!-- Keep Vue's component mount target stable when OverlayScrollbars moves its children. -->
				<div class="app-sidebar-content w-full min-w-0 min-h-full flex flex-col">
					<SidebarContent
						:bedrock="isBedrock"
						:layout="sidebarLayout"
						@visit-hosting="openUrl(companySiteUrl).catch(handleError)"
					/>
				</div>
			</div>
		</div>
	</div>
	<I18nDebugPanel />
	<NotificationPanel :has-sidebar="sidebarVisible" />
	<PopupNotificationPanel :has-sidebar="sidebarVisible" />
	<ErrorModal ref="errorModal" />
	<LaunchLinkConfirmModal ref="launchLinkConfirmModal" />
	<MinecraftAuthErrorModal ref="minecraftAuthErrorModal" />
	<MinecraftRequiredModal ref="minecraftRequiredModal" />
	<ContentInstallModal
		ref="modInstallModal"
		:instances="contentInstallInstances"
		:compatible-loaders="contentInstallLoaders"
		:game-versions="contentInstallGameVersions"
		:loading="contentInstallLoading"
		:default-tab="contentInstallDefaultTab"
		:preferred-loader="contentInstallPreferredLoader"
		:preferred-game-version="contentInstallPreferredGameVersion"
		:release-game-versions="contentInstallReleaseGameVersions"
		:project-info="contentInstallProjectInfo"
		:randomize-icon="randomizeCreationIcon"
		:customize-icon="customizeContentInstallIcon"
		@install="handleInstallToInstance"
		@create-and-install="handleCreateAndInstall"
		@navigate="handleContentInstallNavigate"
		@cancel="handleContentInstallCancel"
	/>
	<ModpackAlreadyInstalledModal
		ref="modpackAlreadyInstalledModal"
		@create-anyway="handleModpackDuplicateCreateAnyway"
		@go-to-instance="handleModpackDuplicateGoToInstance"
	/>
	<ContentUpdaterModal
		ref="incompatibilityWarningModal"
		mode="incompatibility-warning"
		:versions="contentInstallIncompatibilityWarningVersions"
		:current-game-version="contentInstallIncompatibilityWarningCurrentGameVersion"
		:current-loader="contentInstallIncompatibilityWarningCurrentLoader"
		current-version-id=""
		:is-app="true"
		:project-type="contentInstallIncompatibilityWarningProjectType"
		:project-icon-url="contentInstallIncompatibilityWarningProjectIconUrl"
		:project-name="contentInstallIncompatibilityWarningProjectName"
		:warning="contentInstallIncompatibilityWarningMessage"
		:action-loading="contentInstallIncompatibilityWarningInstalling"
		@update="handleContentInstallIncompatibilityWarningInstall"
		@cancel="handleContentInstallIncompatibilityWarningCancel"
	/>
	<ModpackAlreadyInstalledModal
		ref="contentInstallModpackAlreadyInstalledModal"
		@create-anyway="handleContentInstallModpackDuplicateCreateAnyway"
		@go-to-instance="handleContentInstallModpackDuplicateGoToInstance"
	/>
</template>

<style lang="scss" scoped>
.app-grid-layout,
.app-contents {
	--top-bar-height: 3rem;
	--right-bar-width: 20.25rem;
}

.app-grid-layout {
	display: grid;
	grid-template: 'status status' 'nav dummy';
	grid-template-columns: auto 1fr;
	grid-template-rows: auto minmax(0, 1fr);
	position: relative;
	//z-index: 0;
	background-color: var(--color-raised-bg);
	height: 100vh;
}

.app-grid-navbar {
	grid-area: nav;
	min-height: 0;
	position: relative;
	z-index: 2;
	transition: width 0.2s ease;
	overflow-x: hidden;

	> :deep(*) {
		flex-shrink: 0;
	}
}

.app-grid-statusbar {
	grid-area: status;
	padding-right: var(--window-controls-width, 0px);
	position: relative;
	z-index: 2;
}

[data-tauri-drag-region-exclude] {
	-webkit-app-region: no-drag;
}

.app-contents {
	--right-bar-width: 0px;
	position: absolute;
	z-index: 1;
	left: var(--left-bar-width);
	top: var(--top-bar-height);
	right: 0;
	bottom: 0;
	height: calc(100vh - var(--top-bar-height));
	background-color: var(--color-bg);
	border-top-left-radius: var(--radius-xl);
	transition: left 0.2s ease;

	display: grid;
	grid-template-columns: 1fr 0px;
	// transition: grid-template-columns 0.4s ease-in-out;

	&.sidebar-enabled {
		--right-bar-width: 20.25rem;
		grid-template-columns: minmax(0, 1fr) var(--right-bar-width);
	}
}

.loading-indicator-container {
	border-top-left-radius: var(--radius-xl);
	overflow: hidden;
}

.app-sidebar {
	overflow: visible;
	width: var(--right-bar-width);
	position: relative;
	height: calc(100vh - var(--top-bar-height));
	background: var(--brand-gradient-bg);

	--color-button-bg: var(--brand-gradient-button);
	--surface-4: var(--brand-gradient-button);
	--color-button-bg-hover: var(--brand-gradient-border);
	--surface-5: var(--brand-gradient-border);
	--color-divider: var(--brand-gradient-border);
	--color-divider-dark: var(--brand-gradient-border);
}

.disable-advanced-rendering {
	.app-sidebar::before {
		box-shadow: none;
	}

	&.app-contents::before {
		box-shadow: none;
	}

	*,
	:deep(*) {
		box-shadow: none !important;
		--tw-drop-shadow:;
	}
}

.app-sidebar::before {
	content: '';
	box-shadow: -15px 0 15px -15px rgba(0, 0, 0, 0.1) inset;
	top: 0;
	bottom: 0;
	left: -2rem;
	width: 2rem;
	position: absolute;
	pointer-events: none;
}

.app-viewport {
	flex-grow: 1;
	height: 100%;
	overflow: auto;
	overflow-x: hidden;
	scrollbar-gutter: stable;
}

.app-contents::before {
	z-index: 30;
	content: '';
	position: fixed;
	left: var(--left-bar-width);
	top: var(--top-bar-height);
	right: calc(-1 * var(--left-bar-width));
	bottom: calc(-1 * var(--left-bar-width));
	border-radius: var(--radius-xl);
	box-shadow: 1px 1px 15px rgba(0, 0, 0, 0.1) inset;
	border-color: var(--surface-5);
	border-width: 1px;
	border-style: solid;
	pointer-events: none;
}

html[data-reduced-motion='off'] {
	.nav-button-animated-enter-active {
		transition: all 0.5s cubic-bezier(0.15, 1.4, 0.64, 0.96);
	}

	.nav-button-animated-leave-active {
		transition: all 0.25s ease;
	}

	.nav-button-animated-enter-active {
		position: relative;
	}

	.nav-button-animated-enter-active::before {
		content: '';
		inset: 0;
		border-radius: 100vw;
		background-color: var(--color-brand-highlight);
		position: absolute;
		animation: pop 0.5s ease-in forwards;
		opacity: 0;
	}

	@keyframes pop {
		0% {
			scale: 0.5;
		}
		50% {
			opacity: 0.5;
		}
		100% {
			scale: 1.5;
		}
	}

	.nav-button-animated-enter-from {
		scale: 0.5;
		translate: -2rem 0;
		opacity: 0;
	}

	.nav-button-animated-leave-to {
		scale: 0.75;
		opacity: 0;
	}

	.fade-enter-active {
		transition: 0.25s ease-in-out;
	}

	.fade-enter-from {
		opacity: 0;
	}
}
</style>
<style>
.os-theme-dark,
.os-theme-light {
	--os-handle-bg: var(--color-scrollbar) !important;
	--os-handle-bg-hover: var(--color-scrollbar) !important;
	--os-handle-bg-active: var(--color-scrollbar) !important;
}

.app-grid-statusbar {
	padding-left: 0.25rem;
}

.mac-traffic-lights {
	.app-grid-statusbar {
		padding-left: 5rem;
	}
}

.windows {
	.fake-appbar {
		height: 2.5rem !important;
	}

	.info-card {
		right: 22rem;
	}

	.profile-card {
		right: 8rem;
	}
}
</style>
