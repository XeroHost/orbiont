/**
 * CurseForge as a *data source* for the launcher's one native browse/project
 * UI (see the build plan, Fase 4).
 *
 * There is deliberately no CurseForge-specific UI anywhere: this module maps
 * CurseForge's API onto the same data model the rest of the app already
 * renders (Labrinth-shaped search hits, projects, versions, team members and
 * category tags). The browse page, filters, sort, pagination, project page,
 * versions and gallery are all the launcher's own components — so any change
 * to them applies to every provider at once.
 *
 * CurseForge ids are namespaced so they can flow through the same code paths
 * as Modrinth ids without ever colliding (Modrinth ids are base62, never
 * contain "-"):
 *   project  `cf-<modId>`
 *   version  `cf-<modId>-<fileId>`
 *   team     `cf-<modId>` (same as the project; there's one author list)
 */
import type { Labrinth } from '@modrinth/api-client'
import { registerCategoryIconAliases } from '@modrinth/assets'
import { invoke } from '@tauri-apps/api/core'

export type CurseforgeProjectType = 'modpack' | 'mod' | 'resourcepack' | 'datapack' | 'shader'

const PREFIX = 'cf-'

// CurseForge class ids for Minecraft (gameId 432).
const CLASS_IDS: Record<CurseforgeProjectType, number> = {
	modpack: 4471,
	mod: 6,
	resourcepack: 12,
	datapack: 6945,
	shader: 6552,
}
const PROJECT_TYPE_BY_CLASS_ID = Object.fromEntries(
	Object.entries(CLASS_IDS).map(([type, id]) => [id, type]),
) as Record<number, CurseforgeProjectType>

// CurseForge's ModLoaderType enum <-> the launcher's loader tag names.
const LOADER_BY_TYPE: Record<number, string> = {
	1: 'forge',
	4: 'fabric',
	5: 'quilt',
	6: 'neoforge',
}
const LOADER_API_NAMES: Record<string, string> = {
	forge: 'Forge',
	fabric: 'Fabric',
	quilt: 'Quilt',
	neoforge: 'NeoForge',
}
export const CURSEFORGE_LOADERS = Object.keys(LOADER_API_NAMES)

// CurseForge files list loaders and environments alongside game versions.
const NON_VERSION_FILE_TAGS = new Set([
	'forge',
	'fabric',
	'quilt',
	'neoforge',
	'client',
	'server',
	'liteloader',
	'rift',
	'java 8',
	'java 16',
	'java 17',
	'java 21',
])

/** Filter types CurseForge can serve; the rest of the native filters are hidden. */
export function isCurseforgeSupportedFilterType(filterTypeId: string) {
	return (
		filterTypeId.startsWith('category_') ||
		filterTypeId === 'game_version' ||
		filterTypeId === 'mod_loader' ||
		filterTypeId === 'modpack_loader'
	)
}

export const CURSEFORGE_MAX_RESULTS_OPTIONS = [5, 10, 15, 20, 50]
// CurseForge refuses index + pageSize beyond 10,000.
const MAX_REACHABLE_RESULTS = 10000

// The launcher's native sort options mapped onto CurseForge's ModsSearchSortField.
const SORT_FIELDS: Record<string, number> = {
	relevance: 1, // Featured — CurseForge's own "Relevancy"
	downloads: 6, // TotalDownloads
	follows: 2, // Popularity
	newest: 11, // ReleasedDate
	updated: 3, // LastUpdated
}

// Resource pack resolutions use the same "resolutions" header (and so the same
// single-choice filter group) as Modrinth's.
const RESOLUTION_CATEGORIES = new Set(['16x', '32x', '64x', '128x', '256x', '512x and Higher'])

/**
 * CurseForge category name -> the launcher's own category icon, so CurseForge
 * categories render with the same clean line icons as every other category
 * (never CurseForge's logo images).
 */
const CATEGORY_ICONS: Record<string, string> = {
	// Modpacks
	'extra large': 'high',
	'small / light': 'lightweight',
	'combat / pvp': 'combat',
	'sci-fi': 'zap',
	'adventure and rpg': 'adventure',
	'ftb official pack': 'badge-check',
	quests: 'quests',
	tech: 'technology',
	skyblock: 'cloud',
	'map based': 'map-pinned',
	horror: 'skull',
	multiplayer: 'multiplayer',
	'mini game': 'minigame',
	magic: 'magic',
	'vanilla+': 'vanilla-like',
	hardcore: 'heart-crack',
	exploration: 'compass',
	expert: 'challenging',
	rlcraft: 'swords',
	// Mods
	'armor, tools, and weapons': 'equipment',
	'blood magic': 'wand-sparkles',
	create: 'blocks',
	'integrated dynamics': 'network',
	dimensions: 'globe',
	'server utility': 'management',
	'map and information': 'map-pinned',
	addons: 'modded',
	galacticraft: 'target',
	'bug fixes': 'tweaks',
	'applied energistics 2': 'storage',
	kubejs: 'terminal',
	biomes: 'tree-pine',
	'api and library': 'library',
	food: 'food',
	"tinker's construct": 'pickaxe',
	'industrial craft': 'technology',
	farming: 'foliage',
	mobs: 'mobs',
	education: 'scroll-text',
	buildcraft: 'building-2',
	technology: 'technology',
	storage: 'storage',
	automation: 'refresh-ccw',
	crafttweaker: 'terminal',
	'utility & qol': 'utility',
	structures: 'castle',
	'thermal expansion': 'zap',
	'energy, fluid, and item transport': 'transportation',
	redstone: 'zap',
	'modjam 2025': 'trophy',
	forestry: 'tree-pine',
	mcreator: 'modded',
	miscellaneous: 'kitchen-sink',
	genetics: 'paw-print',
	'world gen': 'worldgen',
	performance: 'optimization',
	'twilight forest': 'tree-pine',
	cosmetic: 'palette',
	thaumcraft: 'magic',
	'twitch integration': 'film',
	'ores and resources': 'pickaxe',
	'player transport': 'transportation',
	energy: 'zap',
	processing: 'gauge',
	'refined storage': 'storage',
	creativemode: 'palette',
	"farmer's delight": 'food',
	// Resource packs
	'photo realistic': 'realistic',
	traditional: 'vanilla-like',
	'font packs': 'fonts',
	'mod support': 'modded',
	medieval: 'castle',
	'data packs': 'blocks',
	animated: 'film',
	modern: 'building-2',
	steampunk: 'gauge',
	// Data packs
	adventure: 'adventure',
	library: 'library',
	utility: 'utility',
	fantasy: 'fantasy',
	// Shaders
	realistic: 'realistic',
	vanilla: 'vanilla-like',
}
registerCategoryIconAliases(CATEGORY_ICONS)

// ---------------------------------------------------------------------------
// ids

export function isCurseforgeId(id: string | null | undefined): boolean {
	return typeof id === 'string' && id.startsWith(PREFIX)
}

function projectIdOf(modId: number) {
	return `${PREFIX}${modId}`
}

function versionIdOf(modId: number, fileId: number) {
	return `${PREFIX}${modId}-${fileId}`
}

function parseProjectId(id: string): number {
	const modId = Number.parseInt(id.slice(PREFIX.length), 10)
	if (!Number.isInteger(modId) || modId <= 0) throw new Error(`Invalid CurseForge id: ${id}`)
	return modId
}

function parseVersionId(id: string): { modId: number; fileId: number } {
	const [modId, fileId] = id
		.slice(PREFIX.length)
		.split('-')
		.map((part) => Number.parseInt(part, 10))
	if (!Number.isInteger(modId) || !Number.isInteger(fileId)) {
		throw new Error(`Invalid CurseForge version id: ${id}`)
	}
	return { modId, fileId }
}

// ---------------------------------------------------------------------------
// raw API

interface CfCategory {
	id: number
	name: string
	slug: string
	classId?: number
	parentCategoryId?: number
	isClass?: boolean
}

interface CfFile {
	id: number
	modId: number
	displayName: string
	fileName: string
	releaseType: 1 | 2 | 3
	fileDate: string
	fileLength: number
	downloadCount: number
	downloadUrl: string | null
	gameVersions: string[]
	hashes: { value: string; algo: 1 | 2 }[]
	isServerPack?: boolean
}

interface CfFileIndex {
	gameVersion: string
	fileId: number
	modLoader?: number | null
}

interface CfMod {
	id: number
	name: string
	slug: string
	summary: string
	classId: number
	links: {
		websiteUrl?: string | null
		wikiUrl?: string | null
		issuesUrl?: string | null
		sourceUrl?: string | null
	}
	downloadCount: number
	categories: CfCategory[]
	authors: { id: number; name: string; url: string; avatarUrl?: string | null }[]
	logo: { thumbnailUrl?: string; url?: string } | null
	screenshots: { title: string; description: string; thumbnailUrl: string; url: string }[]
	latestFiles: CfFile[]
	latestFilesIndexes: CfFileIndex[]
	dateCreated: string
	dateModified: string
	dateReleased: string
	allowModDistribution: boolean | null
}

async function api<T>(path: string, query: Record<string, string | undefined> = {}): Promise<T> {
	const pairs = Object.entries(query).filter((entry): entry is [string, string] => !!entry[1])
	return await invoke('plugin:orbiont|orbiont_curseforge_api', { path, query: pairs })
}

const modCache = new Map<number, Promise<CfMod>>()
function getMod(modId: number): Promise<CfMod> {
	let pending = modCache.get(modId)
	if (!pending) {
		pending = api<{ data: CfMod }>(`mods/${modId}`).then((body) => body.data)
		pending.catch(() => modCache.delete(modId))
		modCache.set(modId, pending)
	}
	return pending
}

const MAX_FILE_PAGES = 3
const FILE_PAGE_SIZE = 50
const filesCache = new Map<number, Promise<CfFile[]>>()
function getFiles(modId: number): Promise<CfFile[]> {
	let pending = filesCache.get(modId)
	if (!pending) {
		pending = (async () => {
			const files: CfFile[] = []
			for (let page = 0; page < MAX_FILE_PAGES; page++) {
				const body = await api<{ data: CfFile[]; pagination: { totalCount: number } }>(
					`mods/${modId}/files`,
					{ index: String(page * FILE_PAGE_SIZE), pageSize: String(FILE_PAGE_SIZE) },
				)
				files.push(...body.data)
				if (files.length >= body.pagination.totalCount || body.data.length < FILE_PAGE_SIZE) break
			}
			return files.sort((a, b) => Date.parse(b.fileDate) - Date.parse(a.fileDate))
		})()
		pending.catch(() => filesCache.delete(modId))
		filesCache.set(modId, pending)
	}
	return pending
}

const categoriesCache = new Map<CurseforgeProjectType, Promise<CfCategory[]>>()
function getCategories(projectType: CurseforgeProjectType): Promise<CfCategory[]> {
	let pending = categoriesCache.get(projectType)
	if (!pending) {
		pending = api<{ data: CfCategory[] }>('categories', {
			classId: String(CLASS_IDS[projectType]),
		}).then((body) => body.data.filter((category) => !category.isClass))
		pending.catch(() => categoriesCache.delete(projectType))
		categoriesCache.set(projectType, pending)
	}
	return pending
}

// ---------------------------------------------------------------------------
// mapping helpers

function iconOf(mod: CfMod) {
	return mod.logo?.thumbnailUrl || mod.logo?.url || null
}

function projectTypeOf(mod: CfMod): CurseforgeProjectType {
	return PROJECT_TYPE_BY_CLASS_ID[mod.classId] ?? 'mod'
}

function categoriesOf(mod: CfMod): string[] {
	return [...new Set(mod.categories.filter((c) => !c.isClass).map((c) => c.name))]
}

function loadersOf(mod: CfMod): string[] {
	return [
		...new Set(
			mod.latestFilesIndexes
				.map((index) => (index.modLoader ? LOADER_BY_TYPE[index.modLoader] : undefined))
				.filter((loader): loader is string => !!loader),
		),
	]
}

function gameVersionsOf(mod: CfMod): string[] {
	return [...new Set(mod.latestFilesIndexes.map((index) => index.gameVersion))]
}

function fileGameVersions(file: CfFile) {
	return file.gameVersions.filter((tag) => !NON_VERSION_FILE_TAGS.has(tag.toLowerCase()))
}

function fileLoaders(file: CfFile) {
	return file.gameVersions.map((tag) => tag.toLowerCase()).filter((tag) => tag in LOADER_API_NAMES)
}

/**
 * CurseForge omits downloadUrl when the author disabled third-party
 * distribution. We respect that: such files have no URL here and the
 * install flows send the user to the CurseForge page instead.
 */
function fileDownloadUrl(file: CfFile) {
	return file.downloadUrl ?? ''
}

// ---------------------------------------------------------------------------
// tags (categories) — same shape as Modrinth's, so the native filter sidebar
// builds CurseForge's category groups (include/exclude, multi-select, icons)
// exactly like Modrinth's.

export async function getCurseforgeCategoryTags(
	projectType: CurseforgeProjectType,
): Promise<Labrinth.Tags.v2.Category[]> {
	const categories = await getCategories(projectType)
	const seen = new Set<string>()
	return categories
		.filter((category) => !seen.has(category.name) && seen.add(category.name))
		.map((category) => ({
			name: category.name,
			icon: '',
			project_type: projectType,
			header:
				projectType === 'resourcepack' && RESOLUTION_CATEGORIES.has(category.name)
					? 'resolutions'
					: 'categories',
		}))
}

// ---------------------------------------------------------------------------
// search

export interface CurseforgeSearchFilter {
	type: string
	option: string
	negative?: boolean
}

export async function searchCurseforge(options: {
	projectType: CurseforgeProjectType
	query: string
	sort: string
	limit: number
	page: number
	filters: CurseforgeSearchFilter[]
}): Promise<{ hits: CurseforgeSearchHit[]; totalHits: number }> {
	const categories = await getCategories(options.projectType)
	const categoryIdByName = new Map(categories.map((category) => [category.name, category.id]))

	const includedCategories: string[] = []
	const excludedCategories = new Set<string>()
	const loaders: string[] = []
	const excludedLoaders = new Set<string>()
	const gameVersions: string[] = []
	const excludedProjectIds = new Set<string>()

	for (const filter of options.filters) {
		if (filter.type.startsWith('category_')) {
			if (filter.negative) excludedCategories.add(filter.option)
			else includedCategories.push(filter.option)
		} else if (filter.type === 'mod_loader' || filter.type === 'modpack_loader') {
			if (!LOADER_API_NAMES[filter.option]) continue
			if (filter.negative) excludedLoaders.add(filter.option)
			else loaders.push(filter.option)
		} else if (filter.type === 'game_version' && !filter.negative) {
			gameVersions.push(filter.option)
		} else if (filter.type === 'project_id' && filter.negative) {
			excludedProjectIds.add(filter.option.replace(/^project_id:/, ''))
		}
	}

	const includedCategoryIds = includedCategories
		.map((name) => categoryIdByName.get(name))
		.filter((id): id is number => id !== undefined)

	const limit = Math.min(options.limit, 50)
	const index = Math.min((options.page - 1) * limit, MAX_REACHABLE_RESULTS - limit)

	const body = await api<{ data: CfMod[]; pagination: { totalCount: number } }>('mods/search', {
		classId: String(CLASS_IDS[options.projectType]),
		searchFilter: options.query || undefined,
		sortField: String(SORT_FIELDS[options.sort] ?? SORT_FIELDS.relevance),
		sortOrder: 'desc',
		categoryIds: includedCategoryIds.length
			? JSON.stringify(includedCategoryIds.slice(0, 10))
			: undefined,
		modLoaderTypes: loaders.length
			? JSON.stringify(loaders.slice(0, 5).map((loader) => LOADER_API_NAMES[loader]))
			: undefined,
		gameVersions: gameVersions.length ? JSON.stringify(gameVersions.slice(0, 4)) : undefined,
		index: String(Math.max(0, index)),
		pageSize: String(limit),
	})

	// CurseForge has no exclusion filters and ORs multiple categories, where
	// the native filters AND categories and support excluding — apply those
	// semantics to the returned page so the shared filter UI means the same
	// thing for every provider.
	const hits = body.data
		.map((mod) => toSearchHit(mod, options.projectType))
		.filter((hit) => {
			if (excludedProjectIds.has(hit.project_id)) return false
			if (hit.categories.some((category) => excludedCategories.has(category))) return false
			if (hit.loaders.length && hit.loaders.every((loader) => excludedLoaders.has(loader))) {
				return false
			}
			return includedCategories.every((category) => hit.categories.includes(category))
		})

	return {
		hits,
		totalHits: Math.min(body.pagination.totalCount, MAX_REACHABLE_RESULTS),
	}
}

/** A native search hit, plus the CurseForge website page for "open in browser". */
export type CurseforgeSearchHit = Labrinth.Search.v3.ResultSearchProject & {
	page_url: string | null
}

function toSearchHit(mod: CfMod, projectType: CurseforgeProjectType): CurseforgeSearchHit {
	const categories = categoriesOf(mod)
	return {
		project_id: projectIdOf(mod.id),
		project_types: [projectType],
		all_project_types: [projectType],
		slug: projectIdOf(mod.id),
		author: mod.authors[0]?.name ?? '',
		author_id: null,
		organization: null,
		organization_id: null,
		name: mod.name,
		summary: mod.summary,
		categories,
		display_categories: categories,
		downloads: mod.downloadCount,
		// CurseForge's API doesn't expose follower counts (thumbsUpCount is always
		// 0), so it's unknown rather than a misleading 0 — the UI hides unknowns.
		follows: null as unknown as number,
		icon_url: iconOf(mod),
		date_created: mod.dateCreated,
		date_modified: mod.dateModified,
		license: '',
		gallery: mod.screenshots.map((shot) => shot.url),
		featured_gallery: null,
		color: null,
		loaders: loadersOf(mod),
		project_loader_fields: { game_versions: gameVersionsOf(mod) },
		disclosure_types: [],
		page_url: mod.links.websiteUrl ?? null,
	}
}

// ---------------------------------------------------------------------------
// projects / versions / team — the same shapes the native project page,
// versions list and install flows already consume.

export async function getCurseforgeProject(id: string): Promise<Labrinth.Projects.v2.Project> {
	const modId = parseProjectId(id)
	const [mod, files, description] = await Promise.all([
		getMod(modId),
		getFiles(modId),
		api<{ data: string }>(`mods/${modId}/description`)
			.then((body) => body.data)
			.catch(() => ''),
	])
	const projectType = projectTypeOf(mod)
	const icon = iconOf(mod)

	return {
		id: projectIdOf(mod.id),
		slug: projectIdOf(mod.id),
		project_type: projectType,
		team: projectIdOf(mod.id),
		organization: null,
		title: mod.name,
		description: mod.summary,
		body: description,
		body_url: null,
		published: mod.dateCreated,
		updated: mod.dateModified,
		approved: mod.dateCreated,
		queued: null,
		status: 'approved',
		requested_status: null,
		moderator_message: null,
		license: { id: 'LicenseRef-Unknown', name: '', url: null },
		client_side: 'unknown',
		server_side: 'unknown',
		downloads: mod.downloadCount,
		followers: null,
		categories: categoriesOf(mod),
		additional_categories: [],
		game_versions: [...new Set(files.flatMap(fileGameVersions))],
		loaders: [...new Set([...files.flatMap(fileLoaders), ...loadersOf(mod)])],
		versions: files.map((file) => versionIdOf(mod.id, file.id)),
		icon_url: icon,
		raw_icon_url: icon,
		issues_url: mod.links.issuesUrl ?? null,
		source_url: mod.links.sourceUrl ?? null,
		wiki_url: mod.links.wikiUrl ?? null,
		discord_url: null,
		donation_urls: [],
		gallery: mod.screenshots.map((shot, ordering) => ({
			url: shot.url,
			raw_url: shot.url,
			featured: false,
			title: shot.title,
			description: shot.description,
			created: mod.dateModified,
			ordering,
		})),
		color: null,
		thread_id: '',
		monetization_status: 'demonetized',
		page_url: mod.links.websiteUrl ?? null,
	} as unknown as Labrinth.Projects.v2.Project
}

export async function getCurseforgeProjectV3(id: string) {
	const project = await getCurseforgeProject(id)
	return {
		...project,
		name: project.title,
		summary: project.description,
		project_types: [project.project_type],
		minecraft_server: null,
		minecraft_java_server: null,
		minecraft_bedrock_server: null,
		link_urls: {},
	}
}

function toVersion(file: CfFile, projectType: CurseforgeProjectType): Labrinth.Versions.v2.Version {
	const sha1 = file.hashes.find((hash) => hash.algo === 1)?.value
	const loaders = fileLoaders(file)
	return {
		id: versionIdOf(file.modId, file.id),
		project_id: projectIdOf(file.modId),
		author_id: '',
		featured: false,
		name: file.displayName,
		version_number: file.displayName,
		changelog: '',
		changelog_url: null,
		date_published: file.fileDate,
		downloads: file.downloadCount,
		version_type: file.releaseType === 1 ? 'release' : file.releaseType === 2 ? 'beta' : 'alpha',
		status: 'listed',
		requested_status: null,
		files: [
			{
				hashes: sha1 ? { sha1 } : {},
				url: fileDownloadUrl(file),
				filename: file.fileName,
				primary: true,
				size: file.fileLength,
				file_type: null,
			},
		],
		dependencies: [],
		game_versions: fileGameVersions(file),
		loaders:
			loaders.length > 0
				? loaders
				: projectType === 'resourcepack'
					? ['minecraft']
					: projectType === 'datapack'
						? ['datapack']
						: projectType === 'shader'
							? ['iris', 'optifine']
							: [],
	} as unknown as Labrinth.Versions.v2.Version
}

export async function getCurseforgeVersions(
	versionIds: string[],
): Promise<Labrinth.Versions.v2.Version[]> {
	const byMod = new Map<number, Set<number>>()
	for (const id of versionIds) {
		const { modId, fileId } = parseVersionId(id)
		if (!byMod.has(modId)) byMod.set(modId, new Set())
		byMod.get(modId)!.add(fileId)
	}

	const versions: Labrinth.Versions.v2.Version[] = []
	for (const [modId, fileIds] of byMod) {
		const [mod, files] = await Promise.all([getMod(modId), getFiles(modId)])
		const projectType = projectTypeOf(mod)
		const known = new Map(files.map((file) => [file.id, file]))
		for (const fileId of fileIds) {
			const file =
				known.get(fileId) ??
				(await api<{ data: CfFile }>(`mods/${modId}/files/${fileId}`).then((body) => body.data))
			versions.push(toVersion(file, projectType))
		}
	}
	return versions
}

export async function getCurseforgeVersion(versionId: string) {
	const { modId, fileId } = parseVersionId(versionId)
	const [version] = await getCurseforgeVersions([versionId])
	const changelog = await api<{ data: string }>(`mods/${modId}/files/${fileId}/changelog`)
		.then((body) => body.data)
		.catch(() => '')
	return { ...version, changelog }
}

export async function getCurseforgeTeam(teamId: string) {
	const mod = await getMod(parseProjectId(teamId))
	return mod.authors.map((author, index) => ({
		team_id: teamId,
		user: {
			id: `${PREFIX}user-${author.id}`,
			username: author.name,
			avatar_url: author.avatarUrl ?? null,
			bio: null,
			created: mod.dateCreated,
			role: 'developer',
		},
		role: index === 0 ? 'Owner' : 'Member',
		is_owner: index === 0,
		accepted: true,
		ordering: index,
		permissions: null,
		payouts_split: 0,
	}))
}

/** The download for one CurseForge version (file), for the install flows. */
export async function getCurseforgeVersionFile(versionId: string) {
	const [version] = await getCurseforgeVersions([versionId])
	const file = version.files[0]
	if (!file.url) {
		const mod = await getMod(parseVersionId(versionId).modId)
		throw new CurseforgeDistributionError(mod.name, mod.links.websiteUrl ?? null)
	}
	return { url: file.url, filename: file.filename }
}

async function downloadVersionFile(versionId: string): Promise<string> {
	const { url, filename } = await getCurseforgeVersionFile(versionId)
	return await invoke('plugin:orbiont|orbiont_download_search_result', {
		url,
		fileNameHint: filename,
	})
}

/** Adds a CurseForge version to an instance, for the native install flows. */
export async function installCurseforgeVersionToInstance(
	instanceId: string,
	versionId: string,
	contentType?: string,
): Promise<string> {
	const path = await downloadVersionFile(versionId)
	const projectType = contentType === 'shader' ? 'shaderpack' : contentType
	return await invoke('plugin:instance|instance_add_project_from_path', {
		instanceId,
		projectPath: path,
		projectType,
	})
}

/** A modpack file left out because only curseforge.com may distribute it. */
export interface CurseforgeSkippedFile {
	name: string
	pageUrl: string | null
}

type SkippedFilesListener = (packName: string, files: CurseforgeSkippedFile[]) => void
const skippedFilesListeners = new Set<SkippedFilesListener>()

/** Notified when a converted modpack had to leave files out. */
export function onCurseforgeSkippedFiles(listener: SkippedFilesListener) {
	skippedFilesListeners.add(listener)
	return () => skippedFilesListeners.delete(listener)
}

/**
 * Downloads a CurseForge modpack version and converts it into an `.mrpack`
 * (see `theseus::curseforge_pack`), so it installs through the native
 * modpack installer like any other pack. Returns the `.mrpack` path.
 */
export async function downloadCurseforgeModpack(
	versionId: string,
	/** Omit to skip reporting left-out files (e.g. for a preview). */
	packName?: string,
): Promise<string> {
	const zipPath = await downloadVersionFile(versionId)
	const converted = await invoke<{ path: string; skipped: CurseforgeSkippedFile[] }>(
		'plugin:orbiont|orbiont_convert_curseforge_pack',
		{ path: zipPath },
	)
	if (packName !== undefined && converted.skipped.length > 0) {
		for (const listener of skippedFilesListeners) listener(packName, converted.skipped)
	}
	return converted.path
}

/** The author disabled third-party downloads; only curseforge.com can serve the file. */
export class CurseforgeDistributionError extends Error {
	constructor(
		projectName: string,
		readonly pageUrl: string | null,
	) {
		super(
			`The author of ${projectName} only allows downloading it from CurseForge's website` +
				(pageUrl ? `: ${pageUrl}` : '.'),
		)
	}
}

/** The CurseForge website page for a project, for "open in browser" actions. */
export async function getCurseforgeProjectUrl(id: string) {
	const mod = await getMod(parseProjectId(id))
	return mod.links.websiteUrl ?? null
}

export async function getCurseforgeProjectType(id: string) {
	return projectTypeOf(await getMod(parseProjectId(id)))
}
