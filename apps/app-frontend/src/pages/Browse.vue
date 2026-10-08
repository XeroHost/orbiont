<script setup lang="ts">
import type { Labrinth } from '@orbiont/api-client'
import {
	CheckIcon,
	ClipboardCopyIcon,
	CompassIcon,
	ExternalIcon,
	GlobeIcon,
	PlusIcon,
	SpinnerIcon,
} from '@orbiont/assets'
import type { BrowseInstallContentType, CardAction, ProjectType, Tags } from '@orbiont/ui'
import {
	BrowsePageLayout,
	BrowseSidebar,
	Button,
	commonMessages,
	ContextMenu,
	defineMessages,
	formatProjectTypeSentence,
	getLatestMatchingInstallVersion,
	getSelectedInstallPreferences,
	getTargetInstallPreferences,
	injectNotificationManager,
	NavTabs,
	preferencesDiffer,
	provideBrowseManager,
	resolveInstallPlan,
	useBrowseSearch,
	useDebugLogger,
	useVIntl,
} from '@orbiont/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { openUrl } from '@tauri-apps/plugin-opener'
import type { Ref } from 'vue'
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import type { LocationQuery, RouteLocationNormalizedLoaded } from 'vue-router'
import { useRoute, useRouter } from 'vue-router'

import { useAppEvent } from '@/composables/use-app-event'
import { useAppSettings } from '@/composables/use-app-settings.ts'
import { useCurseforgeCategories } from '@/composables/use-curseforge-categories'
import { config } from '@/config'
import { bedrockFilterLayout } from '@/helpers/bedrock-catalog'
import { bedrockCatalogMessages } from '@/helpers/bedrock-catalog-messages'
import { get_project, get_search_results_v3, get_version_many } from '@/helpers/cache.js'
import type { CurseforgeProjectType, CurseforgeSearchHit } from '@/helpers/curseforge'
import {
	CURSEFORGE_LOADERS,
	CURSEFORGE_MAX_RESULTS_OPTIONS,
	getBedrockGameVersions,
	getCurseforgeProjectVersions,
	isCurseforgeId,
	isCurseforgeSupportedFilterType,
	searchCurseforge,
} from '@/helpers/curseforge'
import {
	get_installed_project_ids as getInstalledProjectIds,
	getInstanceIconUrl,
	list as listInstances,
} from '@/helpers/instance'
import { get as getSettings, set as setSettings } from '@/helpers/settings.ts'
import { get_categories, get_game_versions, get_loaders } from '@/helpers/tags'
import { instanceDetailQueryOptions, instanceKeys } from '@/pages/instance/query-options'
import { type BreadcrumbDefinition, injectBreadcrumbManager } from '@/providers/breadcrumbs'
import { injectContentInstall } from '@/providers/content-install'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const { install: installVersion } = injectContentInstall()
const queryClient = useQueryClient()
const debugLog = useDebugLogger('Browse')

const router = useRouter()
const route = useRoute()
const bedrockBrowse = route.query.edition === 'bedrock'
if (
	bedrockBrowse &&
	!['mod', 'resourcepack', 'world', 'datapack'].includes(String(route.params.projectType))
) {
	void router.replace({ path: '/browse/mod', query: { edition: 'bedrock', src: 'curseforge' } })
}
// Read the current category directly; retain its breadcrumb when opening a project.
// A scheduled copy of the route can leave the header behind the selected tab.
const displayedBrowseRoute = computed<RouteLocationNormalizedLoaded>((previous) => {
	const current = router.currentRoute.value
	return current.path.startsWith('/browse/') ? current : (previous ?? current)
})
const breadcrumbMessages = defineMessages({
	discoverProjectType: {
		id: 'app.browse.discover-project-type',
		defaultMessage: 'Discover {projectType}',
	},
})
const breadcrumbLabel = computed(() => {
	const browseRoute = displayedBrowseRoute.value
	if (bedrockBrowse && browseRoute.params.projectType === 'mod')
		return formatMessage(bedrockCatalogMessages.discoverAddons)
	if (bedrockBrowse && browseRoute.params.projectType === 'datapack')
		return formatMessage(bedrockCatalogMessages.discoverScripts)

	return formatMessage(breadcrumbMessages.discoverProjectType, {
		projectType: formatProjectTypeSentence(
			formatMessage,
			String(browseRoute.params.projectType ?? ''),
			2,
		),
	})
})
const appSettings = useAppSettings()
const browseRouteActive = computed(() => route.path.startsWith('/browse/'))

// Rutas antiguas del catálogo de servidores: la pestaña worlds ya no enlaza al
// catálogo. Si se llega con ?from=worlds o /browse/server, se redirige a
// contenido permitido limpiando esos parámetros heredados.
if (route.params.projectType === 'server' || route.query.from === 'worlds') {
	const nextQuery = { ...route.query }
	delete nextQuery.from
	router.replace({
		path: '/browse/modpack',
		query: nextQuery,
	})
}

const initialInstanceId = computed(() => (bedrockBrowse ? '' : String(route.query.i ?? '')))
const instanceQuery = useQuery(
	computed(() => ({
		...instanceDetailQueryOptions(initialInstanceId.value),
		enabled: !!initialInstanceId.value,
	})),
)
const instance = computed(() => instanceQuery.data.value ?? null)
const installedProjectIds: Ref<string[] | null> = ref(null)
const instanceHideInstalled = ref(route.query.ai === 'true')
const newlyInstalled = ref<string[]>([])
const hiddenInstanceProjectIds = ref<Set<string>>(new Set())
const hiddenInstanceProjectIdsInitialized = ref(false)

const breadcrumbManager = injectBreadcrumbManager()
const instanceBreadcrumbDefinition = {
	slot: 'instance',
	id: () => `instance:${String(displayedBrowseRoute.value.query.i ?? '')}`,
	label: () => instance.value?.name ?? formatMessage(commonMessages.loadingLabel),
	visual: () => ({
		type: 'image' as const,
		src: getInstanceIconUrl(instance.value?.icon_path),
		alt: instance.value?.name,
		tintBy: String(displayedBrowseRoute.value.query.i ?? ''),
	}),
	to: () => {
		return `/instance/${encodeURIComponent(String(displayedBrowseRoute.value.query.i ?? ''))}`
	},
} satisfies BreadcrumbDefinition
const breadcrumbDefinition = {
	slot: 'browse',
	id: () =>
		`browse:${String(displayedBrowseRoute.value.params.projectType ?? '')}:${String(
			displayedBrowseRoute.value.query.i ?? '',
		)}:${String(displayedBrowseRoute.value.query.from ?? '')}`,
	label: breadcrumbLabel,
	to: () => displayedBrowseRoute.value.fullPath,
	visual: { type: 'icon', component: CompassIcon },
} satisfies BreadcrumbDefinition

function syncBreadcrumbs() {
	if (displayedBrowseRoute.value.query.i) {
		const instanceBreadcrumb = breadcrumbManager.reset(instanceBreadcrumbDefinition)
		breadcrumbManager.push(breadcrumbDefinition, { parent: instanceBreadcrumb })
		return
	}

	breadcrumbManager.reset(breadcrumbDefinition)
}

watch(displayedBrowseRoute, syncBreadcrumbs, { immediate: true, flush: 'sync' })

debugLog('fetching tags (categories, loaders, gameVersions)')
const [categories, loaders, availableGameVersions] = await Promise.all([
	(bedrockBrowse ? Promise.resolve([]) : get_categories())
		.catch(handleError)
		.then(ref<Labrinth.Tags.v2.Category[]>),
	(bedrockBrowse ? Promise.resolve([]) : get_loaders())
		.catch(handleError)
		.then(ref<Labrinth.Tags.v2.Loader[]>),
	(bedrockBrowse ? getBedrockGameVersions() : get_game_versions())
		.catch(handleError)
		.then(ref<Labrinth.Tags.v2.GameVersion[]>),
])

const projectType = ref<ProjectType>(
	(route.params.projectType === 'server' ? 'modpack' : route.params.projectType) as ProjectType,
)

// The browse UI is the same for
// every source; the source only decides where search results, categories and
// project data come from (see helpers/curseforge.ts).
const CURSEFORGE_PROJECT_TYPES: string[] = [
	'modpack',
	'mod',
	'resourcepack',
	'datapack',
	'shader',
	'world',
]
const contentSource = ref<'modrinth' | 'curseforge'>(
	bedrockBrowse || route.query.src === 'curseforge' || projectType.value === 'world'
		? 'curseforge'
		: 'modrinth',
)
const supportsCurseforge = computed(() => CURSEFORGE_PROJECT_TYPES.includes(projectType.value))
const useCurseforge = computed(
	() => supportsCurseforge.value && contentSource.value === 'curseforge',
)
const curseforgeCategoryQuery = useCurseforgeCategories(
	projectType,
	computed(() => useCurseforge.value && browseRouteActive.value),
	bedrockBrowse ? 'bedrock' : 'java',
)
watch(curseforgeCategoryQuery.error, (error) => {
	if (error) handleError(error)
})

const tags: Ref<Tags> = computed(() => {
	if (useCurseforge.value) {
		return {
			gameVersions: availableGameVersions.value ?? [],
			loaders: (loaders.value ?? []).filter((loader) => CURSEFORGE_LOADERS.includes(loader.name)),
			categories: curseforgeCategoryQuery.data.value ?? [],
		}
	}
	return {
		gameVersions: availableGameVersions.value ?? [],
		loaders: loaders.value ?? [],
		categories: categories.value ?? [],
	}
})

const allInstalledIds = computed(
	() => new Set([...newlyInstalled.value, ...(installedProjectIds.value ?? [])]),
)

function syncHiddenInstanceProjectIds() {
	hiddenInstanceProjectIds.value = new Set([
		...(installedProjectIds.value ?? []),
		...newlyInstalled.value,
	])
	hiddenInstanceProjectIdsInitialized.value = true
}

watch(
	installedProjectIds,
	(ids) => {
		if (!ids) return
		if (!hiddenInstanceProjectIdsInitialized.value) {
			syncHiddenInstanceProjectIds()
		}
	},
	{ immediate: true },
)

await initInstanceContext()

async function refreshInstalledProjectIds() {
	if (bedrockBrowse) return
	if (!route.query.i) {
		const instances = await queryClient
			.fetchQuery({
				queryKey: [...instanceKeys.all, 'installed-project-ids'],
				queryFn: listInstances,
				staleTime: 0,
			})
			.catch(handleError)
		if (!instances) return

		const ids = instances
			.map((gameInstance) => gameInstance.link?.project_id)
			.filter((id): id is string => !!id)
		debugLog('installedInstanceProjectIds loaded', { count: ids.length })
		installedProjectIds.value = ids
		return
	}

	const targetInstanceId = route.query.i as string
	const ids = await queryClient
		.fetchQuery({
			queryKey: instanceKeys.installedProjectIds(targetInstanceId, 'content'),
			queryFn: () => getInstalledProjectIds(targetInstanceId),
			staleTime: 0,
		})
		.catch(handleError)
	if (!ids) return

	debugLog('installedProjectIds loaded', { count: ids.length })
	installedProjectIds.value = ids
}

async function initInstanceContext() {
	if (bedrockBrowse) return
	debugLog('initInstanceContext', {
		queryI: route.query.i,
		queryAi: route.query.ai,
		queryFrom: route.query.from,
	})
	await Promise.all([
		refreshInstalledProjectIds(),
		route.query.i ? instanceQuery.suspense().catch(handleError) : Promise.resolve(),
	])

	if (route.query.i) {
		debugLog('instance loaded', {
			name: instance.value?.name,
			loader: instance.value?.loader,
			gameVersion: instance.value?.game_version,
		})
	}
}

function setBrowseHideInstalledFlag(flag: 'hide_installed_modpacks', value: boolean) {
	appSettings.featureFlags[flag] = value
	getSettings()
		.then((settings) => {
			settings.feature_flags[flag] = value
			return setSettings(settings)
		})
		.catch(handleError)
}

const hideInstalledModpacks = computed({
	get: () => appSettings.getFeatureFlag('hide_installed_modpacks'),
	set: (value: boolean) => setBrowseHideInstalledFlag('hide_installed_modpacks', value),
})

const instanceFilters = computed(() => {
	const filters = []

	if (instance.value && projectType.value !== 'resourcepack') {
		const isVanillaShader = projectType.value === 'shader' && instance.value.loader === 'vanilla'
		const gameVersion = instance.value.game_version
		if (gameVersion && !isVanillaShader) {
			filters.push({ type: 'game_version', option: gameVersion })
		}

		const platform = instance.value.loader
		const supportedModLoaders = ['fabric', 'forge', 'quilt', 'neoforge']

		if (platform && projectType.value === 'mod' && supportedModLoaders.includes(platform)) {
			filters.push({ type: 'mod_loader', option: platform })
		}
		if (isVanillaShader) {
			filters.push({ type: 'shader_loader', option: 'vanilla' })
		}
	}

	if (
		(instance.value || projectType.value === 'modpack') &&
		(projectType.value === 'modpack' ? hideInstalledModpacks.value : instanceHideInstalled.value) &&
		hiddenInstanceProjectIds.value.size > 0
	) {
		for (const id of hiddenInstanceProjectIds.value) {
			filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
		}
	}

	return filters
})

const combinedProvidedFilters = instanceFilters

// Menú contextual genérico para los resultados (abrir/copiar enlace del
// proyecto). Antes vivía en use-app-server-browse, retirado con el catálogo.
const contextMenuRef = ref<{ open: (event: MouseEvent, options: unknown[]) => void } | null>(null)

function handleRightClick(event: MouseEvent, result: Labrinth.Search.v3.ResultSearchProject) {
	const projectType = result.project_types?.[0] ?? 'project'
	const url = `${config.siteUrl}/${projectType}/${result.slug ?? result.project_id}`
	contextMenuRef.value?.open(event, [
		{
			id: 'open_link',
			label: formatMessage(commonMessages.openInModrinthButton),
			icon: GlobeIcon,
			action: () => void openUrl(url),
		},
		{
			id: 'copy_link',
			label: formatMessage(commonMessages.copyLinkButton),
			icon: ClipboardCopyIcon,
			action: () => void navigator.clipboard.writeText(url),
		},
	])
}

const offline = ref(!navigator.onLine)
const handleOffline = () => {
	debugLog('went offline')
	offline.value = true
}
const handleOnline = () => {
	debugLog('went online')
	offline.value = false
}
window.addEventListener('offline', handleOffline)
window.addEventListener('online', handleOnline)

onBeforeUnmount(() => {
	window.removeEventListener('offline', handleOffline)
	window.removeEventListener('online', handleOnline)
})

const messages = defineMessages({
	projectActionsLabel: {
		id: 'app.browse.project-actions.label',
		defaultMessage: 'Project actions',
	},
	addToAnInstance: {
		id: 'app.browse.add-to-an-instance',
		defaultMessage: 'Add to an instance',
	},
	gameVersionProvidedByInstance: {
		id: 'search.filter.locked.instance-game-version.title',
		defaultMessage: 'Game version is provided by the instance',
	},
	hideInstalledModpacks: {
		id: 'app.browse.hide-installed-modpacks',
		defaultMessage: 'Hide already installed',
	},
	backToInstance: {
		id: 'app.browse.back-to-instance',
		defaultMessage: 'Back to instance',
	},
	modLoaderProvidedByInstance: {
		id: 'search.filter.locked.instance-loader.title',
		defaultMessage: 'Loader is provided by the instance',
	},
	modpacksProjectType: {
		id: 'app.browse.project-type.modpacks',
		defaultMessage: 'Modpacks',
	},
	modsProjectType: { id: 'app.browse.project-type.mods', defaultMessage: 'Mods' },
	resourcePacksProjectType: {
		id: 'app.browse.project-type.resource-packs',
		defaultMessage: 'Resource Packs',
	},
	dataPacksProjectType: {
		id: 'app.browse.project-type.data-packs',
		defaultMessage: 'Data Packs',
	},
	shadersProjectType: { id: 'app.browse.project-type.shaders', defaultMessage: 'Shaders' },
	worldsProjectType: { id: 'project-type.world.category', defaultMessage: 'Worlds' },
	providedByInstance: {
		id: 'search.filter.locked.instance',
		defaultMessage: 'Provided by the instance',
	},
	syncFilterButton: {
		id: 'search.filter.locked.instance.sync',
		defaultMessage: 'Sync with instance',
	},
	openInCurseforge: {
		id: 'app.browse.open-in-curseforge',
		defaultMessage: 'Open in CurseForge',
	},
})

function resetInstanceContext() {
	debugLog('instance context removed, resetting')
	installedProjectIds.value = null
	instanceHideInstalled.value = false
	newlyInstalled.value = []
	hiddenInstanceProjectIds.value = new Set()
	hiddenInstanceProjectIdsInitialized.value = false
	syncBreadcrumbs()
	void refreshInstalledProjectIds()
}

watch(
	() => route.params.projectType as ProjectType,
	async (newType) => {
		if (!browseRouteActive.value) {
			return
		}

		if (!newType || newType === projectType.value) return

		if (newType === 'server') {
			const nextQuery = { ...route.query }
			delete nextQuery.from
			await router.replace({ path: '/browse/modpack', query: nextQuery })
			return
		}

		debugLog('projectType route param changed', { from: projectType.value, to: newType })
		projectType.value = newType
	},
)

watch(
	() => route.query.i,
	async (nextInstanceId, previousInstanceId) => {
		if (!route.path.startsWith('/browse') || nextInstanceId === previousInstanceId) return
		if (!nextInstanceId) {
			resetInstanceContext()
			return
		}

		installedProjectIds.value = null
		hiddenInstanceProjectIdsInitialized.value = false
		await Promise.all([instanceQuery.suspense().catch(handleError), refreshInstalledProjectIds()])
	},
)

const selectableProjectTypes = computed(() => {
	if (bedrockBrowse)
		return [
			{
				label: formatMessage(bedrockCatalogMessages.addons),
				href: '/browse/mod?edition=bedrock&src=curseforge',
			},
			{
				label: formatMessage(messages.resourcePacksProjectType),
				href: '/browse/resourcepack?edition=bedrock&src=curseforge',
			},
			{
				label: formatMessage(messages.worldsProjectType),
				href: '/browse/world?edition=bedrock&src=curseforge',
			},
			{
				label: formatMessage(bedrockCatalogMessages.scripts),
				href: '/browse/datapack?edition=bedrock&src=curseforge',
			},
		]
	let dataPacks = false,
		mods = false,
		modpacks = false

	if (instance.value) {
		if (
			availableGameVersions.value &&
			availableGameVersions.value.findIndex((x) => x.version === instance.value?.game_version) <=
				availableGameVersions.value.findIndex((x) => x.version === '1.13')
		) {
			dataPacks = true
		}

		if (instance.value.loader !== 'vanilla') {
			mods = true
		}
	} else {
		dataPacks = true
		mods = true
		modpacks = true
	}

	const params: LocationQuery = {}

	if (route.query.i) params.i = route.query.i
	if (route.query.ai) params.ai = route.query.ai

	const queryString = new URLSearchParams(params as Record<string, string>).toString()
	const suffix = queryString ? `?${queryString}` : ''

	return [
		{
			label: formatMessage(messages.modpacksProjectType),
			href: `/browse/modpack${suffix}`,
			shown: modpacks,
		},
		{ label: formatMessage(messages.modsProjectType), href: `/browse/mod${suffix}`, shown: mods },
		{
			label: formatMessage(messages.resourcePacksProjectType),
			href: `/browse/resourcepack${suffix}`,
		},
		{
			label: formatMessage(messages.dataPacksProjectType),
			href: `/browse/datapack${suffix}`,
			shown: dataPacks,
		},
		{ label: formatMessage(messages.shadersProjectType), href: `/browse/shader${suffix}` },
		{
			label: formatMessage(messages.worldsProjectType),
			href: `/browse/world${suffix ? `${suffix}&` : '?'}src=curseforge`,
		},
	]
})

const installContext = computed(() => {
	if (instance.value) {
		return {
			name: instance.value.name,
			loader: instance.value.loader,
			gameVersion: instance.value.game_version,
			iconSrc: getInstanceIconUrl(instance.value.icon_path),
			backUrl: `/instance/${encodeURIComponent(instance.value.id)}`,
			backLabel: formatMessage(messages.backToInstance),
			heading: formatMessage(commonMessages.installingContentLabel),
			warning: undefined,
		}
	}
	return null
})

const installingProjectIds = ref<Set<string>>(new Set())

function setProjectInstalling(projectId: string, installing: boolean) {
	const next = new Set(installingProjectIds.value)
	if (installing) {
		next.add(projectId)
	} else {
		next.delete(projectId)
	}
	installingProjectIds.value = next
}

function getCurrentSelectedInstallPreferences(projectTypeValue: string) {
	return getSelectedInstallPreferences({
		contentType: projectTypeValue,
		selectedFilters: searchState.currentFilters.value,
		providedFilters: combinedProvidedFilters.value,
		overriddenProvidedFilterTypes: searchState.overriddenProvidedFilterTypes.value,
	})
}

function getInstanceInstallTargetPreferences(projectTypeValue: string) {
	return getTargetInstallPreferences(
		{
			gameVersion: instance.value?.game_version,
			loader: instance.value?.loader,
		},
		projectTypeValue,
	)
}

async function getInstallProjectVersions(projectId: string) {
	if (isCurseforgeId(projectId)) return getCurseforgeProjectVersions(projectId)
	const project = await get_project(projectId, 'must_revalidate')
	return (await get_version_many(
		project.versions,
		'must_revalidate',
	)) as Labrinth.Versions.v2.Version[]
}

async function chooseInstanceInstallVersion(
	project: Labrinth.Search.v3.ResultSearchProject,
	projectTypeValue: string,
) {
	const targetInstance = instance.value
	if (!targetInstance) {
		return { versionId: null as string | null }
	}

	const selectedPreferences = getCurrentSelectedInstallPreferences(projectTypeValue)
	const targetPreferences = getInstanceInstallTargetPreferences(projectTypeValue)
	if (!preferencesDiffer(selectedPreferences, targetPreferences)) {
		return { versionId: null as string | null }
	}

	const selectedVersion = getLatestMatchingInstallVersion(
		await getInstallProjectVersions(project.project_id),
		selectedPreferences,
	)

	if (!selectedVersion) {
		return { versionId: null as string | null }
	}

	return { versionId: selectedVersion.id }
}

async function chooseFilterMatchingInstallVersion(
	project: Labrinth.Search.v3.ResultSearchProject,
	projectTypeValue: string,
) {
	const plan = await resolveInstallPlan({
		project: {
			project_id: project.project_id,
			title: project.name,
			icon_url: project.icon_url,
		},
		contentType: projectTypeValue as BrowseInstallContentType,
		selectedFilters: searchState.currentFilters.value,
		providedFilters: combinedProvidedFilters.value,
		overriddenProvidedFilterTypes: searchState.overriddenProvidedFilterTypes.value,
		targetPreferences: {},
		getProjectVersions: getInstallProjectVersions,
	})

	return { versionId: plan.versionId }
}

function getCardActions(
	result: Labrinth.Search.v3.ResultSearchProject,
	currentProjectType: string,
): CardAction[] {
	const projectResult = result as Labrinth.Search.v3.ResultSearchProject & {
		installed?: boolean
		installing?: boolean
	}
	const isInstalled =
		projectResult.installed || allInstalledIds.value.has(projectResult.project_id || '')
	const isInstalling = installingProjectIds.value.has(projectResult.project_id)
	const showAsInstalled = isInstalled && !['modpack', 'world'].includes(currentProjectType)

	const isModpack = projectResult.project_types?.includes('modpack')
	const shouldUseInstallIcon = bedrockBrowse || !!instance.value || isModpack

	return [
		{
			key: 'install',
			label: formatMessage(
				isInstalling
					? commonMessages.installingLabel
					: showAsInstalled
						? commonMessages.installedLabel
						: shouldUseInstallIcon
							? commonMessages.installButton
							: messages.addToAnInstance,
			),
			icon: isInstalling ? SpinnerIcon : showAsInstalled ? CheckIcon : PlusIcon,
			iconClass: isInstalling ? 'animate-spin' : undefined,
			disabled: showAsInstalled || isInstalling,
			color: 'brand',
			type: 'outlined',
			onClick: async () => {
				setProjectInstalling(projectResult.project_id, true)
				try {
					const selectedInstall =
						bedrockBrowse || currentProjectType === 'world'
							? { versionId: null as string | null }
							: instance.value
								? await chooseInstanceInstallVersion(projectResult, currentProjectType)
								: isModpack
									? await chooseFilterMatchingInstallVersion(projectResult, currentProjectType)
									: { versionId: null as string | null }
					if (selectedInstall === null) {
						setProjectInstalling(projectResult.project_id, false)
						return
					}
					const selectedPreferences = getCurrentSelectedInstallPreferences(currentProjectType)
					await installVersion(
						projectResult.project_id,
						selectedInstall.versionId,
						instance.value ? instance.value.id : null,
						'SearchCard',
						(versionId, installedProjectIds) => {
							setProjectInstalling(projectResult.project_id, false)
							if (versionId && !bedrockBrowse) {
								onSearchResultsInstalled(installedProjectIds ?? [projectResult.project_id])
							}
						},
						(profile) => {
							router.push(`/instance/${profile}`)
						},
						{
							preferredLoader: instance.value?.loader ?? selectedPreferences.loaders?.[0],
							preferredGameVersion:
								instance.value?.game_version ?? selectedPreferences.gameVersions?.[0],
						},
					)
				} catch (err) {
					setProjectInstalling(projectResult.project_id, false)
					handleError(err)
				}
			},
		},
	]
}

function onSearchResultInstalled(id: string) {
	if (!newlyInstalled.value.includes(id)) {
		newlyInstalled.value = [...newlyInstalled.value, id]
	}
}

function onSearchResultsInstalled(ids: string[]) {
	newlyInstalled.value = Array.from(new Set([...newlyInstalled.value, ...ids]))
}

function markInstalled<T extends Labrinth.Search.v3.ResultSearchProject>(hit: T) {
	const mapped: T & { installed?: boolean } = { ...hit }
	if (instance.value || projectType.value === 'modpack') {
		const installedIds = new Set([...newlyInstalled.value, ...(installedProjectIds.value ?? [])])
		mapped.installed = installedIds.has(hit.project_id)
	}
	return mapped
}

async function searchCurseforgeSource() {
	const overridden = searchState.overriddenProvidedFilterTypes.value
	const provided = combinedProvidedFilters.value.filter(
		(filter) => !overridden.includes(filter.type),
	)
	const params = {
		edition: bedrockBrowse ? ('bedrock' as const) : ('java' as const),
		projectType: projectType.value as CurseforgeProjectType,
		query: searchState.query.value,
		sort: searchState.effectiveCurrentSortType.value.name,
		limit: searchState.maxResults.value,
		page: searchState.currentPage.value,
		filters: [
			...searchState.currentFilters.value.filter(
				(filter) => !provided.some((providedFilter) => providedFilter.type === filter.type),
			),
			...provided,
		],
	}
	const result = await searchCurseforge(params)
	return {
		projectHits: result.hits.map(markInstalled),
		total_hits: result.totalHits,
		per_page: params.limit,
	}
}

async function search(requestParams: string) {
	if (useCurseforge.value) {
		debugLog('searching curseforge', requestParams)
		return await searchCurseforgeSource()
	}
	debugLog('searching v3', requestParams)

	const rawResults = await queryClient.fetchQuery({
		queryKey: ['search', 'v3', requestParams],
		queryFn: () =>
			get_search_results_v3(requestParams, 'must_revalidate') as Promise<{
				result: Labrinth.Search.v3.SearchResults & {
					hits: (Labrinth.Search.v3.ResultSearchProject & { installed?: boolean })[]
				}
			} | null>,
		staleTime: 30_000,
	})

	if (!rawResults) {
		return {
			projectHits: [],
			total_hits: 0,
			per_page: 20,
		}
	}

	for (const hit of rawResults.result.hits) {
		for (const identifier of [hit.project_id, hit.slug]) {
			if (identifier) {
				queryClient.setQueryData(['projects', 'summary', identifier], hit)
			}
		}
	}

	const hits = rawResults.result.hits.map(markInstalled)

	return {
		projectHits: hits,
		total_hits: rawResults.result.total_hits,
		per_page: rawResults.result.hits_per_page,
	}
}

const lockedFilterMessages = computed(() => ({
	gameVersion: formatMessage(messages.gameVersionProvidedByInstance),
	modLoader: formatMessage(messages.modLoaderProvidedByInstance),
	syncButton: formatMessage(messages.syncFilterButton),
	providedBy: formatMessage(messages.providedByInstance),
}))

const maxResultsOptions = computed(() =>
	useCurseforge.value ? CURSEFORGE_MAX_RESULTS_OPTIONS : [5, 10, 15, 20, 50, 100],
)

const searchState = useBrowseSearch({
	projectType,
	tags,
	active: browseRouteActive,
	providedFilters: combinedProvidedFilters,
	search,
	persistentQueryParams: ['i', 'ai', 'src', 'edition'],
	maxResultsOptions,
	getExtraQueryParams: () => ({
		edition: bedrockBrowse ? 'bedrock' : undefined,
		src: useCurseforge.value ? 'curseforge' : undefined,
		ai: instanceHideInstalled.value ? 'true' : undefined,
	}),
})

// The source switch is NavTabs itself (local, non-routing mode) — the exact
// same component as the project-type tabs next to it.
const contentSourceLinks = computed(() => [
	{ label: 'Modrinth', href: 'modrinth', disabled: bedrockBrowse || projectType.value === 'world' },
	{ label: 'CurseForge', href: 'curseforge' },
])
const contentSourceIndex = computed(() => (contentSource.value === 'curseforge' ? 1 : 0))

watch(
	projectType,
	(type) => {
		if (bedrockBrowse || type === 'world') contentSource.value = 'curseforge'
	},
	{ flush: 'sync' },
)

function clearSourceSpecificFilters() {
	// Category options differ per source; loaders and game versions carry over.
	searchState.currentFilters.value = searchState.currentFilters.value.filter(
		(filter) => !filter.type.startsWith('category_'),
	)
}

function onContentSourceTabClick(_index: number, link: { href: string }) {
	const next = link.href === 'curseforge' ? 'curseforge' : 'modrinth'
	if ((bedrockBrowse || projectType.value === 'world') && next === 'modrinth') return
	if (next === contentSource.value) return
	contentSource.value = next
	clearSourceSpecificFilters()
	searchState.currentPage.value = 1
	void searchState.refreshSearch()
}

watch(supportsCurseforge, (supported) => {
	if (!supported && contentSource.value === 'curseforge') {
		contentSource.value = 'modrinth'
		clearSourceSpecificFilters()
		void searchState.refreshSearch()
	}
})

function handleResultContextMenu(
	event: MouseEvent,
	result: Labrinth.Search.v3.ResultSearchProject,
) {
	if (!isCurseforgeId(result.project_id)) {
		handleRightClick(event, result)
		return
	}
	const url = (result as CurseforgeSearchHit).page_url
	if (!url) return
	contextMenuRef.value?.open(event, [
		{
			id: 'open_link',
			label: formatMessage(messages.openInCurseforge),
			icon: GlobeIcon,
			action: () => void openUrl(url),
		},
		{
			id: 'copy_link',
			label: formatMessage(commonMessages.copyLinkButton),
			icon: ClipboardCopyIcon,
			action: () => void navigator.clipboard.writeText(url),
		},
	])
}

watch(
	[() => searchState.query.value, () => searchState.currentFilters.value, () => projectType.value],
	() => {
		if (instance.value || projectType.value === 'modpack') {
			syncHiddenInstanceProjectIds()
		}
	},
	{ deep: true },
)

void searchState.refreshSearch()

useAppEvent('instance', async (event) => {
	if (event.event === 'created' || event.event === 'removed') {
		if (!route.query.i) {
			await refreshInstalledProjectIds()
			if (projectType.value === 'modpack') {
				if (event.event === 'removed') {
					syncHiddenInstanceProjectIds()
				}
				await searchState.refreshSearch()
			}
		}
	}

	if (instance.value && event.instance_id === instance.value.id && event.event === 'synced') {
		await refreshInstalledProjectIds()
		await searchState.refreshSearch()
	}
})

function getProjectBrowseQuery() {
	if (!browseRouteActive.value) {
		return undefined
	}
	if (!installContext.value && !bedrockBrowse) return undefined
	const { from: _legacyFrom, ...restQuery } = route.query
	return {
		...restQuery,
		b: route.fullPath,
	}
}

const advancedFiltersCollapsed = computed({
	get: () => appSettings.getFeatureFlag('advanced_filters_collapsed'),
	set: (value) => {
		appSettings.featureFlags['advanced_filters_collapsed'] = value
		getSettings()
			.then((settings) => {
				settings.feature_flags['advanced_filters_collapsed'] = value
				return setSettings(settings)
			})
			.catch(handleError)
	},
})

const dismissedPhotosensitivityFilterWarning = computed({
	get: () => appSettings.getFeatureFlag('dismissed_photosensitivity_filter_warning'),
	set: (value) => {
		appSettings.featureFlags['dismissed_photosensitivity_filter_warning'] = value
		getSettings()
			.then((settings) => {
				settings.feature_flags['dismissed_photosensitivity_filter_warning'] = value
				return setSettings(settings)
			})
			.catch(handleError)
	},
})

provideBrowseManager({
	tags,
	projectType,
	projectTypeDisplayName: computed(() =>
		bedrockBrowse
			? projectType.value === 'mod'
				? formatMessage(bedrockCatalogMessages.addons)
				: projectType.value === 'datapack'
					? formatMessage(bedrockCatalogMessages.scripts)
					: undefined
			: undefined,
	),
	...searchState,
	filters: computed(() =>
		bedrockBrowse ? bedrockFilterLayout(searchState.filters.value) : searchState.filters.value,
	),
	advancedFiltersCollapsed,
	dismissedPhotosensitivityFilterWarning,
	// Orbiont doesn't use Modrinth's content-disclosure taxonomy (AI content,
	// telemetry, paid features, etc.), so that group is always noise. Other
	// sources only hide the native filters they can't serve.
	hiddenFilterTypes: computed(() =>
		useCurseforge.value
			? searchState.filters.value
					.map((filter) => filter.id)
					.filter(
						(id) =>
							!isCurseforgeSupportedFilterType(id) || (bedrockBrowse && id.includes('loader')),
					)
			: ['advanced'],
	),
	maxResultsOptions,
	// Author names are shown but not clickable: author profiles only exist per
	// provider, and fetching them isn't worth the extra API traffic.
	getAuthorLink: () => '',
	getProjectLink: (result: Labrinth.Search.v3.ResultSearchProject) => ({
		path: `/project/${result.project_id ?? result.slug}`,
		query: getProjectBrowseQuery(),
	}),
	selectableProjectTypes,
	showProjectTypeTabs: computed(() => true),
	variant: 'app',
	getCardActions,
	installContext,
	providedFilters: combinedProvidedFilters,
	hideInstalled: computed({
		get: () => {
			if (projectType.value === 'modpack') return hideInstalledModpacks.value
			return instanceHideInstalled.value
		},
		set: (val: boolean) => {
			if (projectType.value === 'modpack') {
				hideInstalledModpacks.value = val
				if (val) syncHiddenInstanceProjectIds()
				return
			}
			instanceHideInstalled.value = val
			if (val) syncHiddenInstanceProjectIds()
		},
	}),
	showHideInstalled: computed(() => projectType.value === 'modpack' || !!instance.value),
	hideInstalledLabel: computed(() =>
		formatMessage(
			projectType.value === 'modpack'
				? messages.hideInstalledModpacks
				: commonMessages.hideInstalledContentLabel,
		),
	),
	onInstalled: onSearchResultInstalled,
	onContextMenu: handleResultContextMenu,
	offline,
	lockedFilterMessages,
})
</script>

<template>
	<div class="flex flex-col gap-2 p-6">
		<BrowsePageLayout>
			<template v-if="supportsCurseforge" #nav-tabs-suffix>
				<NavTabs
					mode="local"
					:links="contentSourceLinks"
					:active-index="contentSourceIndex"
					@tab-click="onContentSourceTabClick"
				/>
			</template>
			<template #after>
				<ContextMenu ref="contextMenuRef" :label="formatMessage(messages.projectActionsLabel)">
					<template #open_link="{ option }">
						<GlobeIcon /> {{ option.label }} <ExternalIcon />
					</template>
				</ContextMenu>
			</template>
		</BrowsePageLayout>
		<Teleport v-if="browseRouteActive" to="#sidebar-teleport-target">
			<BrowseSidebar>
				<template v-if="useCurseforge" #prepend>
					<div
						v-if="curseforgeCategoryQuery.isFetching.value"
						class="flex items-center gap-2 p-3 text-secondary"
						role="status"
					>
						<SpinnerIcon class="size-4 animate-spin" />
						{{ formatMessage(commonMessages.loadingLabel) }}
					</div>
					<Button
						v-else-if="curseforgeCategoryQuery.isError.value"
						class="m-3"
						@click="curseforgeCategoryQuery.refetch()"
					>
						{{ formatMessage(commonMessages.retryButton) }}
					</Button>
				</template>
			</BrowseSidebar>
		</Teleport>
	</div>
</template>
