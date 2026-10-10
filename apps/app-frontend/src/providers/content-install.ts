import type { Labrinth } from '@orbiont/api-client'
import {
	type ContentInstallInstance,
	type ContentInstallProjectInfo,
	type ContentItem,
	createContext,
	defineMessage,
	getLatestMatchingInstallVersion,
	useVIntl,
} from '@orbiont/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import dayjs from 'dayjs'
import { nextTick, type Ref, ref } from 'vue'
import type { Router } from 'vue-router'

import { useAppSettings } from '@/composables/use-app-settings.ts'
import { config } from '@/config'
import { requestBedrockInstall } from '@/helpers/bedrock-catalog'
import {
	get_organization,
	get_project,
	get_project_many,
	get_team,
	get_version_many,
} from '@/helpers/cache.js'
import { getCurseforgeProjectVersions, isCurseforgeId } from '@/helpers/curseforge'
import {
	install_create_instance,
	install_create_modpack_instance,
	installJobInstanceId,
	wait_for_install_job,
} from '@/helpers/install'
import {
	add_project_from_version,
	get,
	get_install_candidates,
	get_projects,
	getInstanceIconUrl,
	install_project_with_dependencies,
	list,
	remove_project,
	type ResolveContentPlan,
} from '@/helpers/instance'
import { get_game_versions } from '@/helpers/tags'
import type { GameInstance, InstanceLoader } from '@/helpers/types'
import { requestWorldInstall } from '@/helpers/world-install'
import type { AppEvents } from '@/providers/app-events'
interface ModalRef {
	show: (initialVersionId?: string) => void
	hide: () => void
}

interface ModpackAlreadyInstalledModalRef {
	show: (instanceName: string, instanceId: string) => void
}

export type ContentInstallCallback = (versionId?: string, installedProjectIds?: string[]) => void
type InstallingProjectDisplay = {
	id?: string
	slug?: string | null
	title?: string
	name?: string
	icon_url?: string | null
	project_type?: string
	type?: string
	organization?: string | null
	team?: string
}
const LOADER_ORDER = ['vanilla', 'fabric', 'quilt', 'neoforge', 'forge']
const SUPPORTED_LOADERS: Set<string> = new Set(['vanilla', 'forge', 'fabric', 'quilt', 'neoforge'])
const VANILLA_COMPATIBLE_LOADERS: Set<string> = new Set(['minecraft', 'datapack'])
const noCompatibleVersionsMessage = defineMessage({
	id: 'app.content-install.no-compatible-versions',
	defaultMessage:
		'No available versions match {compatibilityLabel}. Select a version to install anyway. Dependencies will not be installed automatically.',
})

const RESOLVABLE_PROJECT_TYPES = new Set<Labrinth.Content.v3.ContentType>([
	'mod',
	'plugin',
	'datapack',
	'resourcepack',
	'shader',
	'modpack',
])

function resolveContentType(
	projectType?: Labrinth.Projects.v2.ProjectType,
): Labrinth.Content.v3.ContentType {
	return projectType &&
		projectType !== 'world' &&
		projectType !== 'project' &&
		RESOLVABLE_PROJECT_TYPES.has(projectType)
		? projectType
		: 'mod'
}

function isVersionCompatible(
	version: Labrinth.Versions.v2.Version,
	project: Labrinth.Projects.v2.Project,
	instance: Pick<GameInstance, 'game_version' | 'loader'>,
) {
	return (
		version.game_versions.includes(instance.game_version) &&
		(project.project_type === 'mod'
			? version.loaders.includes(instance.loader) || version.loaders.includes('datapack')
			: true)
	)
}

function findPreferredVersion(
	versions: Labrinth.Versions.v2.Version[],
	project: Labrinth.Projects.v2.Project,
	instance: Pick<GameInstance, 'game_version' | 'loader'>,
) {
	const projectType = project.project_type ?? 'mod'

	return (
		versions.find(
			(v) =>
				v.game_versions.includes(instance.game_version) &&
				(projectType === 'mod' ? v.loaders.includes(instance.loader) : true),
		) ?? versions.find((v) => isVersionCompatible(v, project, instance))
	)
}

function sortLoaders(loaders: string[]): string[] {
	return loaders.slice().sort((a, b) => {
		const aIdx = LOADER_ORDER.indexOf(a)
		const bIdx = LOADER_ORDER.indexOf(b)
		if (aIdx === -1 && bIdx === -1) return a.localeCompare(b)
		if (aIdx === -1) return 1
		if (bIdx === -1) return -1
		return aIdx - bIdx
	})
}

type InstallTargetInstance = Pick<GameInstance, 'id' | 'name' | 'game_version' | 'loader'> & {
	icon_path?: string | null
}

export interface ContentInstallContext {
	instances: Ref<ContentInstallInstance[]>
	compatibleLoaders: Ref<string[]>
	gameVersions: Ref<string[]>
	loading: Ref<boolean>
	defaultTab: Ref<'existing' | 'new'>
	preferredLoader: Ref<string | null>
	preferredGameVersion: Ref<string | null>
	releaseGameVersions: Ref<Set<string>>
	projectInfo: Ref<ContentInstallProjectInfo | null>
	handleInstallToInstance: (instance: ContentInstallInstance) => Promise<void>
	handleCreateAndInstall: (data: {
		optifineInstallerPath?: string | null
		name: string
		iconPath: string | null
		iconPreviewUrl: string | null
		loader: string
		gameVersion: string
	}) => Promise<void>
	prepareNewInstance: (projectId: string) => Promise<void>
	handleNavigate: (instance: ContentInstallInstance) => void
	handleCancel: () => void
	setContentInstallModal: (ref: ModalRef) => void
	setModpackAlreadyInstalledModal: (ref: ModpackAlreadyInstalledModalRef) => void
	handleModpackDuplicateCreateAnyway: () => Promise<void>
	handleModpackDuplicateGoToInstance: (instanceId: string) => void
	setIncompatibilityWarningModal: (ref: ModalRef) => void
	incompatibilityWarningVersions: Ref<Labrinth.Versions.v2.Version[]>
	incompatibilityWarningCurrentGameVersion: Ref<string>
	incompatibilityWarningCurrentLoader: Ref<string>
	incompatibilityWarningProjectType: Ref<string | undefined>
	incompatibilityWarningProjectIconUrl: Ref<string | undefined>
	incompatibilityWarningProjectName: Ref<string | undefined>
	incompatibilityWarningMessage: Ref<string | undefined>
	incompatibilityWarningInstalling: Ref<boolean>
	handleIncompatibilityWarningInstall: (version: Labrinth.Versions.v2.Version) => Promise<void>
	handleIncompatibilityWarningCancel: () => void
	install: (
		projectId: string,
		versionId?: string | null,
		instanceId?: string | null,
		source?: string,
		callback?: ContentInstallCallback,
		createInstanceCallback?: (instanceId: string) => void,
		hints?: { preferredLoader?: string; preferredGameVersion?: string; showProjectInfo?: boolean },
	) => Promise<void>
	installingItems: Ref<Map<string, ContentItem[]>>
	installRevisionByInstance: Ref<Map<string, number>>
	installFailureRevisionByInstance: Ref<Map<string, number>>
}

export const [injectContentInstall, provideContentInstall] = createContext<ContentInstallContext>(
	'root',
	'contentInstall',
)

export function createContentInstall(opts: {
	router: Router
	handleError: (err: unknown) => void
	appEvents: AppEvents
}): ContentInstallContext {
	const { formatMessage } = useVIntl()
	const appSettings = useAppSettings()
	const instances = ref<ContentInstallInstance[]>([])
	const compatibleLoaders = ref<string[]>([])
	const gameVersions = ref<string[]>([])
	const loading = ref(false)
	const defaultTab = ref<'existing' | 'new'>('existing')
	const preferredLoader = ref<string | null>(null)
	const preferredGameVersion = ref<string | null>(null)
	const releaseGameVersions = ref<Set<string>>(new Set())

	const projectInfo = ref<ContentInstallProjectInfo | null>(null)
	const installingItems = ref<Map<string, ContentItem[]>>(new Map())
	const installRevisionByInstance = ref<Map<string, number>>(new Map())
	const installFailureRevisionByInstance = ref<Map<string, number>>(new Map())
	const incompatibilityWarningVersions = ref<Labrinth.Versions.v2.Version[]>([])
	const incompatibilityWarningCurrentGameVersion = ref('')
	const incompatibilityWarningCurrentLoader = ref('')
	const incompatibilityWarningProjectType = ref<string | undefined>(undefined)
	const incompatibilityWarningProjectIconUrl = ref<string | undefined>(undefined)
	const incompatibilityWarningProjectName = ref<string | undefined>(undefined)
	const incompatibilityWarningMessage = ref<string | undefined>(undefined)
	const incompatibilityWarningInstalling = ref(false)

	function addInstallingItem(
		instanceId: string,
		project: {
			id: string
			slug?: string | null
			title: string
			icon_url?: string | null
			project_type?: string
			organization?: string | null
			team?: string
		},
		version?: Labrinth.Versions.v2.Version,
	) {
		const primaryFile = version?.files?.find((f) => f.primary) ?? version?.files?.[0]
		const placeholder: ContentItem = {
			id: `__installing_${project.id}`,
			file_name: `__installing_${project.id}`,
			project: {
				id: project.id,
				slug: project.slug ?? '',
				title: project.title,
				icon_url: project.icon_url ?? undefined,
			},
			version: version
				? {
						id: version.id,
						version_number: version.version_number,
						file_name: primaryFile?.filename ?? '',
					}
				: undefined,
			project_type: project.project_type ?? 'mod',
			has_update: false,
			update_version_id: null,
			enabled: true,
			installing: true,
		}
		const next = new Map(installingItems.value)
		const items = next.get(instanceId) ?? []
		if (items.some((i) => i.file_name === placeholder.file_name)) return
		next.set(instanceId, [...items, placeholder])
		installingItems.value = next

		if (project.organization) {
			get_organization(project.organization)
				.then((org: { id: string; slug: string; name: string; icon_url?: string }) => {
					updateInstallingItem(instanceId, placeholder.file_name, {
						owner: {
							id: org.id,
							name: org.name,
							avatar_url: org.icon_url,
							type: 'organization',
						},
					})
				})
				.catch(() => {})
		} else if (project.team) {
			get_team(project.team)
				.then(
					(
						members: {
							user: { id: string; username: string; avatar_url?: string }
							is_owner: boolean
						}[],
					) => {
						const owner = members.find((m) => m.is_owner)
						if (owner) {
							updateInstallingItem(instanceId, placeholder.file_name, {
								owner: {
									id: owner.user.id,
									name: owner.user.username,
									avatar_url: owner.user.avatar_url,
									type: 'user',
								},
							})
						}
					},
				)
				.catch(() => {})
		}
	}

	function updateInstallingItem(
		instanceId: string,
		fileName: string,
		updates: Partial<ContentItem>,
	) {
		const next = new Map(installingItems.value)
		const items = next.get(instanceId)
		if (!items) return
		const index = items.findIndex((i) => i.file_name === fileName)
		if (index === -1) return
		const updated = [...items]
		updated[index] = { ...updated[index], ...updates }
		next.set(instanceId, updated)
		installingItems.value = next
	}

	function removeInstallingItems(instanceId: string, projectIds: string[]) {
		const next = new Map(installingItems.value)
		const items = next.get(instanceId)
		if (items) {
			const idsToRemove = new Set(projectIds.map((id) => `__installing_${id}`))
			const filtered = items.filter((i) => !idsToRemove.has(i.file_name))
			if (filtered.length > 0) {
				next.set(instanceId, filtered)
			} else {
				next.delete(instanceId)
			}
			installingItems.value = next
		}
	}

	function resolvedProjectIds(plan: ResolveContentPlan) {
		return [
			plan.primary.project_id,
			...plan.dependencies.map((dependency) => dependency.project_id),
		]
	}

	async function addInstallingItemsForPlan(
		instanceId: string,
		plan: ResolveContentPlan,
		primaryProject: Labrinth.Projects.v2.Project,
		primaryVersion: Labrinth.Versions.v2.Version,
	) {
		const entries = [plan.primary, ...plan.dependencies]
		const projectIds = [...new Set(entries.map((entry) => entry.project_id))]
		const versionIds = [...new Set(entries.map((entry) => entry.version_id))]
		const projectMap = new Map<string, InstallingProjectDisplay>([
			[primaryProject.id, primaryProject],
		])
		const versionMap = new Map<string, Labrinth.Versions.v2.Version>([
			[primaryVersion.id, primaryVersion],
		])

		const [projects, versions] = await Promise.all([
			get_project_many(projectIds, 'bypass').catch(() => []),
			get_version_many(versionIds, 'bypass').catch(() => []),
		])

		for (const project of projects as InstallingProjectDisplay[]) {
			if (project?.id) projectMap.set(project.id, project)
		}
		for (const version of versions as Labrinth.Versions.v2.Version[]) {
			if (version?.id) versionMap.set(version.id, version)
		}

		for (const entry of entries) {
			const project = projectMap.get(entry.project_id)
			const version = versionMap.get(entry.version_id)
			addInstallingItem(
				instanceId,
				{
					id: entry.project_id,
					slug: project?.slug ?? entry.project_id,
					title: project?.title ?? project?.name ?? entry.project_id,
					icon_url: project?.icon_url ?? null,
					project_type: project?.project_type ?? project?.type ?? primaryProject.project_type,
					organization: project?.organization ?? null,
					team: project?.team,
				},
				version,
			)
		}
	}

	function markInstanceContentChanged(instanceId: string) {
		const next = new Map(installRevisionByInstance.value)
		next.set(instanceId, (next.get(instanceId) ?? 0) + 1)
		installRevisionByInstance.value = next
	}

	function markInstanceContentInstallFailed(instanceId: string) {
		const next = new Map(installFailureRevisionByInstance.value)
		next.set(instanceId, (next.get(instanceId) ?? 0) + 1)
		installFailureRevisionByInstance.value = next
	}

	opts.appEvents.on('instance', (event) => {
		if (event.event === 'content_install_finished') {
			markInstanceContentChanged(event.instance_id)
			removeInstallingItems(event.instance_id, event.project_ids)
		} else if (event.event === 'content_install_failed') {
			removeInstallingItems(event.instance_id, event.project_ids)
			markInstanceContentInstallFailed(event.instance_id)
			markInstanceContentChanged(event.instance_id)
			opts.handleError(event.message)
		}
	})

	let modalRef: ModalRef | null = null
	let modpackAlreadyInstalledModalRef: ModpackAlreadyInstalledModalRef | null = null
	let incompatibilityWarningModalRef: ModalRef | null = null
	let currentProject: Labrinth.Projects.v2.Project | null = null
	let currentVersions: Labrinth.Versions.v2.Version[] = []
	let currentCallback: ContentInstallCallback = () => {}
	let worldSelectionCommitted = false
	let instanceMap: Record<string, InstallTargetInstance> = {}
	let incompatibilityWarningInstance: InstallTargetInstance | null = null
	let incompatibilityWarningProject: Labrinth.Projects.v2.Project | null = null
	let incompatibilityWarningCallback: ContentInstallCallback = () => {}
	let incompatibilityWarningInstalled = false

	let pendingModpackInstall: {
		project: Labrinth.Projects.v2.Project
		version: string
		source: string
		callback: ContentInstallCallback
		createInstanceCallback: (instanceId: string) => void
	} | null = null

	async function showModInstallModal(
		project: Labrinth.Projects.v2.Project,
		versions: Labrinth.Versions.v2.Version[],
		onInstall: ContentInstallCallback,
		hints?: {
			preferredLoader?: string
			preferredGameVersion?: string
			showProjectInfo?: boolean
			showModal?: boolean
		},
	) {
		currentProject = project
		currentVersions = versions
		let completed = false
		currentCallback = (...args) => {
			if (completed) return
			completed = true
			onInstall(...args)
		}
		worldSelectionCommitted = false

		instances.value = []
		loading.value = true
		defaultTab.value = 'existing'

		let projectInfoPromise: Promise<unknown> = Promise.resolve()
		if (hints?.showProjectInfo) {
			projectInfo.value = {
				title: project.title,
				iconUrl: project.icon_url,
				link: `/project/${project.slug ?? project.id}`,
			}
			if (project.organization) {
				projectInfoPromise = get_organization(project.organization)
					.then((org: { id: string; slug: string; name: string; icon_url?: string }) => {
						if (projectInfo.value) {
							const orgSlug = org.slug ?? org.id
							projectInfo.value = {
								...projectInfo.value,
								owner: {
									name: org.name,
									iconUrl: org.icon_url,
									circle: false,
									link: () => openUrl(`${config.siteUrl}/organization/${orgSlug}`),
								},
							}
						}
					})
					.catch(() => {})
			} else if (project.team) {
				projectInfoPromise = get_team(project.team)
					.then(
						(
							members: {
								user: { id: string; username: string; avatar_url?: string }
								is_owner: boolean
							}[],
						) => {
							const owner = members.find((m) => m.is_owner)
							if (owner && projectInfo.value) {
								projectInfo.value = {
									...projectInfo.value,
									owner: {
										name: owner.user.username,
										iconUrl: owner.user.avatar_url,
										circle: true,
									},
								}
							}
						},
					)
					.catch(() => {})
			}
		} else {
			projectInfo.value = null
		}

		const loaderSet = new Set<string>()
		const gameVersionSet = new Set<string>()
		for (const v of versions) {
			for (const l of v.loaders) loaderSet.add(l)
			for (const gv of v.game_versions) gameVersionSet.add(gv)
		}
		const mappedLoaders = new Set<string>()
		for (const l of loaderSet) {
			if (SUPPORTED_LOADERS.has(l)) mappedLoaders.add(l)
			else if (VANILLA_COMPATIBLE_LOADERS.has(l)) mappedLoaders.add('vanilla')
		}
		compatibleLoaders.value = sortLoaders([...mappedLoaders])
		if (project.project_type === 'world') {
			compatibleLoaders.value = [...LOADER_ORDER]
		}
		gameVersions.value = [...gameVersionSet]
		releaseGameVersions.value = new Set(gameVersionSet)

		preferredLoader.value =
			hints?.preferredLoader && loaderSet.has(hints.preferredLoader) ? hints.preferredLoader : null
		preferredGameVersion.value =
			hints?.preferredGameVersion && gameVersionSet.has(hints.preferredGameVersion)
				? hints.preferredGameVersion
				: null

		await nextTick()
		if (hints?.showModal !== false) {
			modalRef?.show()
		}

		const gameVersionMetadataPromise = get_game_versions()
			.then((allGameVersions) => {
				const releases = new Set<string>()
				const ordered: string[] = []
				for (const gv of allGameVersions) {
					if (gameVersionSet.has(gv.version)) {
						ordered.push(gv.version)
						if (gv.version_type === 'release') {
							releases.add(gv.version)
						}
					}
				}
				gameVersions.value = ordered
				releaseGameVersions.value = releases
			})
			.catch(() => {})

		try {
			const candidates =
				project.project_type === 'world'
					? (await list())
							.filter((item) => !item.quarantined && item.install_stage === 'installed')
							.map((item) => ({
								...item,
								installed: false,
								compatible: versions.some((version) =>
									version.game_versions.includes(item.game_version),
								),
							}))
					: await get_install_candidates(
							project.id,
							project.project_type,
							getInstallTargets(versions),
						)
			const newInstanceMap: Record<string, InstallTargetInstance> = {}
			const newInstances: ContentInstallInstance[] = candidates.map((instance) => {
				newInstanceMap[instance.id] = instance
				return {
					id: instance.id,
					name: instance.name,
					iconUrl: getInstanceIconUrl(instance.icon_path),
					installed: instance.installed,
					compatible: instance.compatible,
					installing: false,
				}
			})

			instanceMap = newInstanceMap
			instances.value = newInstances

			if (!newInstances.some((i) => i.compatible && !i.installed)) {
				defaultTab.value = 'new'
			}
		} catch (err) {
			opts.handleError(err)
		} finally {
			loading.value = false
		}
		await gameVersionMetadataPromise
		await projectInfoPromise
	}

	async function prepareNewInstance(projectId: string) {
		const project: Labrinth.Projects.v2.Project = await get_project(projectId, 'must_revalidate')
		if (!project || project.project_type === 'modpack') {
			throw new Error(`Project cannot be prepared as a new instance: '${projectId}'`)
		}

		const versions = ((await loadInstallVersions(project)) as Labrinth.Versions.v2.Version[]).sort(
			(a, b) => dayjs(b.date_published).valueOf() - dayjs(a.date_published).valueOf(),
		)

		await showModInstallModal(project, versions, () => {}, {
			showProjectInfo: true,
			showModal: false,
		})
	}

	function getInstallTargets(versions: Labrinth.Versions.v2.Version[]) {
		const targets: { game_version: string; loader: string }[] = []
		const seen = new Set<string>()

		for (const version of versions) {
			for (const gameVersion of version.game_versions) {
				for (const loader of version.loaders) {
					const key = `${gameVersion}\0${loader}`
					if (seen.has(key)) continue
					seen.add(key)
					targets.push({ game_version: gameVersion, loader })
				}
			}
		}

		return targets
	}

	async function handleInstallToInstance(instance: ContentInstallInstance) {
		const selectedInstance = instanceMap[instance.id]
		const storeInstance = instances.value.find((i) => i.id === instance.id)
		if (!currentProject || !selectedInstance) {
			opts.handleError('No project or instance found')
			return
		}

		if (currentProject.project_type === 'world') {
			const project = currentProject
			const versions = currentVersions
			const callback = currentCallback
			const version = findPreferredVersion(versions, project, selectedInstance)
			worldSelectionCommitted = true
			modalRef?.hide()
			if (!version) {
				await showIncompatibilityWarning(selectedInstance, project, versions, versions[0], callback)
				return
			}
			if (storeInstance) storeInstance.installing = true
			try {
				await installWorldVersion(selectedInstance.id, project, version, callback)
			} catch (error) {
				callback()
				opts.handleError(error)
			} finally {
				if (storeInstance) storeInstance.installing = false
			}
			return
		}

		const version = findPreferredVersion(currentVersions, currentProject, selectedInstance)
		if (!version) {
			if (currentVersions.length > 0 && incompatibilityWarningModalRef) {
				const onIncompatibleInstall = (versionId?: string) => {
					if (versionId && storeInstance) {
						storeInstance.installed = true
					}
					currentCallback(versionId, versionId && currentProject ? [currentProject.id] : undefined)
				}
				await showIncompatibilityWarning(
					selectedInstance,
					currentProject,
					currentVersions,
					currentVersions[0],
					onIncompatibleInstall,
				)
			} else {
				opts.handleError('No version found')
			}
			return
		}

		if (storeInstance) storeInstance.installing = true

		const installedProjectIds: string[] = [currentProject.id]
		let plannedProjectIds: string[] = [currentProject.id]
		addInstallingItem(instance.id, currentProject, version)

		try {
			const request = {
				project_id: currentProject.id,
				version_id: version.id,
				content_type: resolveContentType(currentProject.project_type),
			}
			const plan = await install_project_with_dependencies(instance.id, request)
			plannedProjectIds = resolvedProjectIds(plan)
			await addInstallingItemsForPlan(instance.id, plan, currentProject, version)
			installedProjectIds.splice(
				0,
				installedProjectIds.length,
				plan.primary.project_id,
				...plan.dependencies.map((dependency) => dependency.project_id),
			)
			if (storeInstance) {
				storeInstance.installed = true
				storeInstance.installing = false
			}
			currentCallback(version.id, installedProjectIds)
		} catch (err) {
			if (storeInstance) storeInstance.installing = false
			removeInstallingItems(instance.id, plannedProjectIds)
			markInstanceContentInstallFailed(instance.id)
			opts.handleError(err)
		}
	}

	async function showIncompatibilityWarning(
		instance: InstallTargetInstance,
		project: Labrinth.Projects.v2.Project,
		versions: Labrinth.Versions.v2.Version[],
		version: Labrinth.Versions.v2.Version,
		callback: ContentInstallCallback,
	) {
		incompatibilityWarningInstance = instance
		incompatibilityWarningProject = project
		incompatibilityWarningCallback = callback
		incompatibilityWarningInstalled = false
		incompatibilityWarningInstalling.value = false
		incompatibilityWarningVersions.value = versions
		incompatibilityWarningCurrentGameVersion.value = instance.game_version ?? ''
		incompatibilityWarningCurrentLoader.value = instance.loader ?? ''
		incompatibilityWarningProjectType.value = project.project_type
		incompatibilityWarningProjectIconUrl.value = project.icon_url ?? undefined
		incompatibilityWarningProjectName.value = project.title

		const compatibilityLabel = ['resourcepack', 'datapack', 'world'].includes(project.project_type)
			? (instance.game_version ?? '')
			: `${instance.loader ?? ''} ${instance.game_version ?? ''}`.trim()
		incompatibilityWarningMessage.value = formatMessage(noCompatibleVersionsMessage, {
			compatibilityLabel,
		})

		await nextTick()
		incompatibilityWarningModalRef?.show(version.id)
	}

	async function handleIncompatibilityWarningInstall(version: Labrinth.Versions.v2.Version) {
		if (!incompatibilityWarningInstance || !incompatibilityWarningProject) return
		if (incompatibilityWarningProject.project_type === 'world') {
			const instance = incompatibilityWarningInstance
			const project = incompatibilityWarningProject
			const callback = incompatibilityWarningCallback
			incompatibilityWarningInstalled = true // The world dialog now owns completion/cancellation.
			incompatibilityWarningModalRef?.hide()
			try {
				await installWorldVersion(instance.id, project, version, callback)
			} catch (error) {
				callback()
				opts.handleError(error)
			}
			return
		}

		incompatibilityWarningInstalling.value = true
		addInstallingItem(incompatibilityWarningInstance.id, incompatibilityWarningProject, version)
		try {
			await add_project_from_version(incompatibilityWarningInstance.id, version.id, 'standalone')
		} catch (err) {
			opts.handleError(err)
			incompatibilityWarningInstalling.value = false
			removeInstallingItems(incompatibilityWarningInstance.id, [incompatibilityWarningProject.id])
			markInstanceContentInstallFailed(incompatibilityWarningInstance.id)
			return
		}

		incompatibilityWarningInstalling.value = false
		incompatibilityWarningInstalled = true
		incompatibilityWarningCallback(version.id, [incompatibilityWarningProject.id])
		markInstanceContentChanged(incompatibilityWarningInstance.id)
		incompatibilityWarningModalRef?.hide()
		removeInstallingItems(incompatibilityWarningInstance.id, [incompatibilityWarningProject.id])
	}

	function handleIncompatibilityWarningCancel() {
		if (!incompatibilityWarningInstalled) {
			incompatibilityWarningCallback()
		}
		incompatibilityWarningInstalled = false
	}

	async function handleCreateAndInstall(data: {
		optifineInstallerPath?: string | null
		name: string
		iconPath: string | null
		iconPreviewUrl: string | null
		loader: string
		gameVersion: string
	}) {
		if (currentProject?.project_type === 'world') {
			const project = currentProject
			const callback = currentCallback
			const version = currentVersions.find((item) => item.game_versions.includes(data.gameVersion))
			if (!version) return
			worldSelectionCommitted = true
			try {
				const job = await install_create_instance({
					name: data.name,
					gameVersion: data.gameVersion,
					loader: data.loader as InstanceLoader,
					loaderVersion: 'latest',
					iconPath: data.iconPath,
				})
				const finished = await wait_for_install_job(opts.appEvents, job.job_id)
				const id = installJobInstanceId(finished)
				if (!id) throw new Error('The new instance was not created')
				await installWorldVersion(id, project, version, callback)
			} catch (error) {
				callback()
				opts.handleError(error)
			}
			return
		}
		const project = currentProject
		const callback = currentCallback
		if (!project) return
		const loaderCandidates =
			data.loader === 'vanilla' ? ['vanilla', 'datapack', 'minecraft'] : [data.loader]
		const version =
			currentVersions.find(
				(v) =>
					v.game_versions.includes(data.gameVersion) &&
					loaderCandidates.some((l) => v.loaders.includes(l)),
			) ?? currentVersions[0]

		let createdInstanceId: string | null = null
		worldSelectionCommitted = true
		try {
			const job = await install_create_instance({
				optifineInstallerPath: data.optifineInstallerPath,
				name: data.name,
				gameVersion: data.gameVersion,
				loader: data.loader as InstanceLoader,
				loaderVersion: 'latest',
				iconPath: data.iconPath,
			})
			const id = installJobInstanceId(job)
			if (!id) throw new Error('The new instance was not created')
			createdInstanceId = id
			await wait_for_install_job(opts.appEvents, job.job_id)
			addInstallingItem(id, project, version)

			const plan = await install_project_with_dependencies(id, {
				project_id: project.id,
				version_id: version.id,
				content_type: resolveContentType(project.project_type),
			})
			await addInstallingItemsForPlan(id, plan, project, version)
			await opts.router.push(`/instance/${encodeURIComponent(id)}`)

			callback(version.id, resolvedProjectIds(plan))
			if (currentCallback === callback) modalRef?.hide()
		} catch (err) {
			if (createdInstanceId) {
				removeInstallingItems(createdInstanceId, [project.id])
				markInstanceContentInstallFailed(createdInstanceId)
			}
			callback()
			if (currentCallback === callback) worldSelectionCommitted = false
			opts.handleError(err)
		}
	}

	function handleNavigate(instance: ContentInstallInstance) {
		modalRef?.hide()
		opts.router.push(`/instance/${encodeURIComponent(instance.id)}`)
	}

	function handleCancel() {
		if (worldSelectionCommitted) return
		currentCallback?.()
	}

	async function installWorldVersion(
		instanceId: string,
		project: Labrinth.Projects.v2.Project,
		version: Labrinth.Versions.v2.Version,
		callback: ContentInstallCallback,
	) {
		const installed = await requestWorldInstall({
			project,
			versions: [version],
			instanceId,
			autoInstall: true,
			allowIncompatible: true,
		})
		callback(installed ?? undefined)
	}

	async function install(
		projectId: string,
		versionId?: string | null,
		instanceId?: string | null,
		source: string = 'unknown',
		callback: ContentInstallCallback = () => {},
		createInstanceCallback: (instanceId: string) => void = () => {},
		hints?: { preferredLoader?: string; preferredGameVersion?: string; showProjectInfo?: boolean },
	) {
		const project: Labrinth.Projects.v2.Project = await get_project(projectId, 'must_revalidate')

		if (!project) {
			opts.handleError(`Project not found: '${projectId}'`)
			return
		}
		if (
			(project as Labrinth.Projects.v2.Project & { minecraft_edition?: string })
				.minecraft_edition === 'bedrock'
		) {
			const installed = await requestBedrockInstall({ project, versionId })
			callback(installed ?? undefined)
			return
		}

		if (project.project_type === 'world') {
			const versions = (
				versionId
					? await get_version_many([versionId], 'must_revalidate')
					: await loadInstallVersions(project)
			).sort((a, b) => dayjs(b.date_published).valueOf() - dayjs(a.date_published).valueOf())
			if (!versions.length) throw new Error(`No versions available for ${project.title}`)
			if (instanceId) {
				const instance = await get(instanceId)
				if (!instance) return
				const version = findPreferredVersion(versions, project, instance)
				if (version) await installWorldVersion(instanceId, project, version, callback)
				else await showIncompatibilityWarning(instance, project, versions, versions[0], callback)
			} else {
				await showModInstallModal(project, versions, callback, hints)
			}
			return
		}

		if (project.project_type === 'modpack') {
			let version = versionId ?? null
			if (!version) {
				const hasHints = !!(hints?.preferredGameVersion || hints?.preferredLoader)
				if (hasHints) {
					const versions = (await loadInstallVersions(project, {
						gameVersion: hints?.preferredGameVersion,
						loader: hints?.preferredLoader,
					})) as Labrinth.Versions.v2.Version[]
					const matching = getLatestMatchingInstallVersion(versions, {
						gameVersions: hints?.preferredGameVersion ? [hints.preferredGameVersion] : undefined,
						loaders: hints?.preferredLoader ? [hints.preferredLoader] : undefined,
					})
					version = matching?.id ?? null
				}
				version ??= project.versions[project.versions.length - 1]
			}
			const packs = await list()
			const existingPack = packs.find((pack) => pack.link?.project_id === project.id)

			if (existingPack && !appSettings.getFeatureFlag('skip_non_essential_warnings')) {
				pendingModpackInstall = { project, version, source, callback, createInstanceCallback }
				modpackAlreadyInstalledModalRef?.show(existingPack.name, existingPack.id)
				return
			}

			const job = await install_create_modpack_instance({
				type: 'fromVersionId',
				project_id: project.id,
				version_id: version,
				title: project.title,
				icon_url: project.raw_icon_url,
			})
			const instanceId = installJobInstanceId(job)
			if (instanceId) {
				createInstanceCallback(instanceId)
			}
			callback(version)
		} else if (instanceId) {
			const [instanceOrNull, instanceProjects] = await Promise.all([
				get(instanceId),
				get_projects(instanceId),
			])
			if (!instanceOrNull) return

			const instance = instanceOrNull
			let versions = versionId
				? await get_version_many([versionId], 'must_revalidate')
				: await loadInstallVersions(project, {
						gameVersion: instance.game_version,
						loader: project.project_type === 'mod' ? instance.loader : undefined,
					})
			if (!versions.length) versions = await get_version_many(project.versions, 'must_revalidate')
			if (!versions.length) throw new Error(`No versions available for ${project.title}`)
			const projectVersions = versions.sort(
				(a, b) => dayjs(b.date_published).valueOf() - dayjs(a.date_published).valueOf(),
			)

			let version = versionId
				? projectVersions.find((v) => v.id === versionId)
				: findPreferredVersion(projectVersions, project, instance)
			if (!version) version = projectVersions[0]

			if (isVersionCompatible(version, project, instance)) {
				for (const [path, file] of Object.entries(instanceProjects)) {
					if (file.metadata?.project_id === project.id) {
						await remove_project(instance.id, path)
					}
				}

				const installedProjectIds: string[] = [project.id]
				let plannedProjectIds: string[] = [project.id]
				addInstallingItem(instanceId, project, version)
				try {
					const request = {
						project_id: project.id,
						version_id: version.id,
						content_type: resolveContentType(project.project_type),
					}
					const plan = await install_project_with_dependencies(instance.id, request)
					plannedProjectIds = resolvedProjectIds(plan)
					await addInstallingItemsForPlan(instanceId, plan, project, version)
					installedProjectIds.splice(
						0,
						installedProjectIds.length,
						plan.primary.project_id,
						...plan.dependencies.map((dependency) => dependency.project_id),
					)

					callback(version.id, installedProjectIds)
				} catch (err) {
					removeInstallingItems(instanceId, plannedProjectIds)
					markInstanceContentInstallFailed(instanceId)
					throw err
				}
			} else {
				await showIncompatibilityWarning(instance, project, projectVersions, version, callback)
			}
		} else {
			let versions = ((await loadInstallVersions(project)) as Labrinth.Versions.v2.Version[]).sort(
				(a, b) => dayjs(b.date_published).valueOf() - dayjs(a.date_published).valueOf(),
			)
			if (versionId) versions = versions.filter((v) => v.id === versionId)
			await showModInstallModal(project, versions, callback, hints)
		}
	}

	return {
		instances,
		compatibleLoaders,
		gameVersions,
		loading,
		defaultTab,
		preferredLoader,
		preferredGameVersion,
		releaseGameVersions,
		projectInfo,
		handleInstallToInstance,
		handleCreateAndInstall,
		prepareNewInstance,
		handleNavigate,
		handleCancel,
		setContentInstallModal(ref: ModalRef) {
			modalRef = ref
		},
		setModpackAlreadyInstalledModal(ref: ModpackAlreadyInstalledModalRef) {
			modpackAlreadyInstalledModalRef = ref
		},
		async handleModpackDuplicateCreateAnyway() {
			if (!pendingModpackInstall) return
			const { project, version, callback, createInstanceCallback } = pendingModpackInstall
			pendingModpackInstall = null
			const job = await install_create_modpack_instance({
				type: 'fromVersionId',
				project_id: project.id,
				version_id: version,
				title: project.title,
				icon_url: project.raw_icon_url,
			})
			const instanceId = installJobInstanceId(job)
			if (instanceId) {
				createInstanceCallback(instanceId)
			}
			callback(version)
		},
		handleModpackDuplicateGoToInstance(instanceId: string) {
			pendingModpackInstall = null
			opts.router.push(`/instance/${encodeURIComponent(instanceId)}`)
		},
		setIncompatibilityWarningModal(ref: ModalRef) {
			incompatibilityWarningModalRef = ref
		},
		incompatibilityWarningVersions,
		incompatibilityWarningCurrentGameVersion,
		incompatibilityWarningCurrentLoader,
		incompatibilityWarningProjectType,
		incompatibilityWarningProjectIconUrl,
		incompatibilityWarningProjectName,
		incompatibilityWarningMessage,
		incompatibilityWarningInstalling,
		handleIncompatibilityWarningInstall,
		handleIncompatibilityWarningCancel,
		install,
		installingItems,
		installRevisionByInstance,
		installFailureRevisionByInstance,
	}
}

async function loadInstallVersions(
	project: Labrinth.Projects.v2.Project,
	options: { gameVersion?: string; loader?: string } = {},
): Promise<Labrinth.Versions.v2.Version[]> {
	return isCurseforgeId(project.id)
		? getCurseforgeProjectVersions(project.id, options)
		: get_version_many(project.versions, 'must_revalidate')
}
