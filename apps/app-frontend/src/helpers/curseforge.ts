/**
 * CurseForge as a *data source* for the launcher's one native browse/project
 * UI.
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
import type { Labrinth } from '@orbiont/api-client'
import { registerCategoryIconAliases } from '@orbiont/assets'
import { invoke } from '@tauri-apps/api/core'

export type CurseforgeProjectType =
	'modpack' | 'mod' | 'resourcepack' | 'datapack' | 'shader' | 'world'

const PREFIX = 'cf-'

// CurseForge class ids for Minecraft (gameId 432).
const CLASS_IDS: Record<CurseforgeProjectType, number> = {
	modpack: 4471,
	mod: 6,
	resourcepack: 12,
	datapack: 6945,
	shader: 6552,
	world: 17,
}
export const BEDROCK_GAME_ID = 78022
export const BEDROCK_CLASS_IDS = {
	mod: 4984,
	resourcepack: 6929,
	world: 6913,
	datapack: 6940,
} as const
export type MinecraftEdition = 'java' | 'bedrock'
function classId(type: CurseforgeProjectType, edition: MinecraftEdition) {
	if (edition === 'java') return CLASS_IDS[type]
	if (!(type in BEDROCK_CLASS_IDS)) throw new Error('Unsupported Bedrock content type')
	return BEDROCK_CLASS_IDS[type as keyof typeof BEDROCK_CLASS_IDS]
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
const RESOLUTION_CATEGORIES = new Set([
	'16x',
	'32x',
	'64x',
	'128x',
	'256x',
	'512x and Higher',
	'x16',
	'x32',
	'x64',
	'x128',
])

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
	// Worlds
	creation: 'blocks',
	'game map': 'map-pinned',
	'modded world': 'modded',
	parkour: 'footprints',
	puzzle: 'game-mechanics',
	survival: 'heart-pulse',
	// Bedrock uses a separate category taxonomy.
	'texture packs': 'palette',
	pvp: 'combat',
	players: 'users',
	maps: 'map-pinned',
	roleplay: 'theater',
	skins: 'users',
	cosmetics: 'decoration',
	'minecraft addon maker': 'modded',
	shaders: 'core-shaders',
	'l 3d packs': 'models',
	'enhanced visuals': 'reflections',
	rollercoaster: 'transportation',
	ctm: 'flag',
	'custom terrain': 'worldgen',
	scripts: 'terminal',
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
	dependencies?: { modId: number; relationType: number }[]
}

// CurseForge FileRelationType: 3 = RequiredDependency.
const REQUIRED_DEPENDENCY = 3

interface CfFileIndex {
	gameVersion: string
	fileId: number
	modLoader?: number | null
}

interface CfMod {
	id: number
	gameId?: number
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
	videos?: { url: string; title?: string; description?: string }[]
	latestFiles: CfFile[]
	latestFilesIndexes: CfFileIndex[]
	dateCreated: string
	dateModified: string
	dateReleased: string
	allowModDistribution: boolean | null
}

const requestsInFlight = new Map<string, Promise<unknown>>()
function request<T>(command: string, args: Record<string, unknown>): Promise<T> {
	const key = JSON.stringify([command, args])
	let pending = requestsInFlight.get(key)
	if (!pending) {
		pending = invoke(command, args).catch((error: unknown) => {
			const message =
				typeof error === 'object' && error !== null && 'message' in error
					? String(error.message)
					: String(error)
			throw new Error(message)
		})
		requestsInFlight.set(key, pending)
		pending.then(
			() => requestsInFlight.delete(key),
			() => requestsInFlight.delete(key),
		)
	}
	return pending as Promise<T>
}

async function api<T>(path: string, query: Record<string, string | undefined> = {}): Promise<T> {
	const pairs = Object.entries(query).filter((entry): entry is [string, string] => !!entry[1])
	return await request('plugin:orbiont|orbiont_curseforge_api', { path, query: pairs })
}

async function batch<T>(
	path: 'mods' | 'mods/files',
	key: 'modIds' | 'fileIds',
	ids: number[],
): Promise<T[]> {
	const data: T[] = []
	const unique = [...new Set(ids)]
	for (let offset = 0; offset < unique.length; offset += 2000) {
		const response = await request<{ data: T[] }>('plugin:orbiont|orbiont_curseforge_api_post', {
			path,
			body: { [key]: unique.slice(offset, offset + 2000) },
		})
		data.push(...response.data)
	}
	return data
}

// Coalesce simultaneous requests without retaining CurseForge API data.
const modInFlight = new Map<number, Promise<CfMod>>()
function getMod(modId: number): Promise<CfMod> {
	let pending = modInFlight.get(modId)
	if (!pending) {
		pending = api<{ data: CfMod }>(`mods/${modId}`).then((body) => body.data)
		pending.then(
			() => modInFlight.delete(modId),
			() => modInFlight.delete(modId),
		)
		modInFlight.set(modId, pending)
	}
	return pending
}

const MAX_FILE_PAGES = 10
const FILE_PAGE_SIZE = 50
const filesInFlight = new Map<string, Promise<CfFile[]>>()
function getFiles(
	modId: number,
	options: { gameVersion?: string; loader?: string } = {},
): Promise<CfFile[]> {
	const key = JSON.stringify([modId, options.gameVersion, options.loader])
	let pending = filesInFlight.get(key)
	if (!pending) {
		pending = (async () => {
			const page = (index: number) =>
				api<{ data: CfFile[]; pagination: { totalCount: number } }>(`mods/${modId}/files`, {
					gameVersion: options.gameVersion,
					modLoaderType: options.loader ? LOADER_TYPE_BY_NAME[options.loader] : undefined,
					index: String(index * FILE_PAGE_SIZE),
					pageSize: String(FILE_PAGE_SIZE),
				})
			const first = await page(0)
			const pages = Math.min(
				MAX_FILE_PAGES,
				Math.ceil(first.pagination.totalCount / FILE_PAGE_SIZE),
			)
			const rest = []
			for (let index = 1; index < pages; index++) rest.push(await page(index))
			const files = [first, ...rest].flatMap((body) => body.data)
			return files.sort((a, b) => Date.parse(b.fileDate) - Date.parse(a.fileDate))
		})()
		pending.then(
			() => filesInFlight.delete(key),
			() => filesInFlight.delete(key),
		)
		filesInFlight.set(key, pending)
	}
	return pending
}

const categoriesInFlight = new Map<string, Promise<CfCategory[]>>()
function getCategories(
	projectType: CurseforgeProjectType,
	edition: MinecraftEdition = 'java',
): Promise<CfCategory[]> {
	const key = `${edition}:${projectType}`
	let pending = categoriesInFlight.get(key)
	if (!pending) {
		pending = api<{ data: CfCategory[] }>('categories', {
			gameId: edition === 'bedrock' ? String(BEDROCK_GAME_ID) : undefined,
			classId: String(classId(projectType, edition)),
		}).then((body) => body.data.filter((category) => !category.isClass))
		pending.then(
			() => categoriesInFlight.delete(key),
			() => categoriesInFlight.delete(key),
		)
		categoriesInFlight.set(key, pending)
	}
	return pending
}

// ---------------------------------------------------------------------------
// mapping helpers

function iconOf(mod: CfMod) {
	return mod.logo?.thumbnailUrl || mod.logo?.url || null
}

function projectTypeOf(mod: CfMod): CurseforgeProjectType {
	if (mod.gameId === BEDROCK_GAME_ID)
		return (Object.entries(BEDROCK_CLASS_IDS).find(([, id]) => id === mod.classId)?.[0] ??
			'mod') as CurseforgeProjectType
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
	edition: MinecraftEdition = 'java',
): Promise<Labrinth.Tags.v2.Category[]> {
	const categories = await getCategories(projectType, edition)
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
	edition?: MinecraftEdition
	query: string
	sort: string
	limit: number
	page: number
	filters: CurseforgeSearchFilter[]
}): Promise<{ hits: CurseforgeSearchHit[]; totalHits: number }> {
	const categories = options.filters.some(
		(filter) => filter.type.startsWith('category_') && !filter.negative,
	)
		? await getCategories(options.projectType, options.edition)
		: []
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
		gameId: options.edition === 'bedrock' ? String(BEDROCK_GAME_ID) : undefined,
		classId: String(classId(options.projectType, options.edition ?? 'java')),
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
		.filter(
			(mod) =>
				options.edition !== 'bedrock' ||
				(mod.gameId === BEDROCK_GAME_ID && mod.classId === classId(options.projectType, 'bedrock')),
		)
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
	minecraft_edition?: MinecraftEdition
}

function toSearchHit(mod: CfMod, projectType: CurseforgeProjectType): CurseforgeSearchHit {
	const categories = categoriesOf(mod)
	return {
		project_id: projectIdOf(mod.id),
		minecraft_edition: mod.gameId === BEDROCK_GAME_ID ? 'bedrock' : 'java',
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
		loaders: mod.gameId === BEDROCK_GAME_ID ? [] : loadersOf(mod),
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
	const [mod, description] = await Promise.all([
		getMod(modId),
		api<{ data: string }>(`mods/${modId}/description`)
			.then((body) => body.data)
			.catch(() => ''),
	])
	const projectType = projectTypeOf(mod)
	const icon = iconOf(mod)

	return {
		id: projectIdOf(mod.id),
		minecraft_edition: mod.gameId === BEDROCK_GAME_ID ? 'bedrock' : 'java',
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
		game_versions: [
			...new Set([...gameVersionsOf(mod), ...mod.latestFiles.flatMap(fileGameVersions)]),
		],
		loaders:
			mod.gameId === BEDROCK_GAME_ID
				? []
				: [...new Set([...mod.latestFiles.flatMap(fileLoaders), ...loadersOf(mod)])],
		versions: [
			...new Set([
				...mod.latestFilesIndexes.map((index) => index.fileId),
				...mod.latestFiles.map((file) => file.id),
			]),
		].map((fileId) => versionIdOf(mod.id, fileId)),
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
		videos: mod.videos ?? [],
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

function toVersion(
	file: CfFile,
	projectType: CurseforgeProjectType,
	bedrock = false,
): Labrinth.Versions.v2.Version {
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
		dependencies: (file.dependencies ?? [])
			.filter((dependency) => dependency.relationType === REQUIRED_DEPENDENCY)
			.map((dependency) => ({
				project_id: projectIdOf(dependency.modId),
				version_id: null,
				file_name: null,
				dependency_type: 'required',
			})),
		game_versions: fileGameVersions(file),
		loaders: bedrock
			? []
			: loaders.length > 0
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
	const requested = versionIds.map(parseVersionId)
	const [files, mods] = await Promise.all([
		batch<CfFile>(
			'mods/files',
			'fileIds',
			requested.map((id) => id.fileId),
		),
		batch<CfMod>(
			'mods',
			'modIds',
			requested.map((id) => id.modId),
		),
	])
	const filesById = new Map(files.map((file) => [file.id, file]))
	const modsById = new Map(mods.map((mod) => [mod.id, mod]))
	return requested.map(({ modId, fileId }) => {
		const file = filesById.get(fileId)
		const mod = modsById.get(modId)
		if (!file || !mod || file.modId !== modId)
			throw new Error(`CurseForge file ${fileId} not found in project ${modId}`)
		return toVersion(file, projectTypeOf(mod), mod.gameId === BEDROCK_GAME_ID)
	})
}

/** Fetch history only when choosing a version; constrain it to the target instance when possible. */
export async function getCurseforgeProjectVersions(
	id: string,
	options: { gameVersion?: string; loader?: string } = {},
) {
	const modId = parseProjectId(id)
	const [mod, files] = await Promise.all([getMod(modId), getFiles(modId, options)])
	return files.map((file) => toVersion(file, projectTypeOf(mod), mod.gameId === BEDROCK_GAME_ID))
}

export async function getBedrockGameVersions(): Promise<Labrinth.Tags.v2.GameVersion[]> {
	const body = await api<{ data: { versions: string[] }[] }>('games/78022/versions')
	return [...new Set(body.data.flatMap((group) => group.versions))]
		.sort((a, b) => b.localeCompare(a, 'en', { numeric: true }))
		.map((version) => ({
			version,
			version_type: 'release',
			date: '',
			major: false,
		}))
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

/**
 * Downloads a CurseForge file. The core resolves it by id through the facade,
 * only accepts CurseForge's CDN over https and verifies the file's SHA-1.
 */
export async function downloadVersionFile(versionId: string): Promise<string> {
	const { modId, fileId } = parseVersionId(versionId)
	try {
		return await request('plugin:orbiont|orbiont_download_curseforge_file', { modId, fileId })
	} catch (error) {
		const manual = parseCurseforgeManualDownload(error)
		if (!manual) throw error
		if (manual.projectId !== modId || manual.fileId !== fileId) throw error
		return requestCurseforgeManualDownload(manual)
	}
}

export interface CurseforgeManualDownload {
	projectId: number
	fileId: number
	fileName: string
	fileSize: number
	sha1: string
	pageUrl: string | null
}

export function parseCurseforgeManualDownload(error: unknown): CurseforgeManualDownload | null {
	const message =
		error instanceof Error
			? error.message
			: typeof error === 'string'
				? error
				: String((error as { message?: unknown })?.message ?? '')
	const marker = 'CURSEFORGE_MANUAL_DOWNLOAD:'
	const position = message.indexOf(marker)
	if (position < 0) return null
	try {
		const file = JSON.parse(message.slice(position + marker.length))
		if (
			!Number.isSafeInteger(file.projectId) ||
			file.projectId <= 0 ||
			!Number.isSafeInteger(file.fileId) ||
			file.fileId <= 0 ||
			typeof file.fileName !== 'string' ||
			!Number.isSafeInteger(file.fileSize) ||
			file.fileSize < 0 ||
			!/^[a-f0-9]{40}$/.test(file.sha1)
		)
			return null
		const url = new URL(file.pageUrl)
		if (
			url.protocol !== 'https:' ||
			url.hostname !== 'www.curseforge.com' ||
			url.port ||
			url.username ||
			url.password ||
			url.search ||
			url.hash ||
			(!url.pathname.startsWith('/minecraft/') &&
				!url.pathname.startsWith('/minecraft-bedrock/')) ||
			!url.pathname.endsWith(`/files/${file.fileId}`)
		)
			return null
		return file
	} catch {
		return null
	}
}

let manualDownloadHandler: ((file: CurseforgeManualDownload) => Promise<string | null>) | undefined
export class CurseforgeDownloadCancelled extends Error {
	constructor(fileName: string) {
		super(`CurseForge manual download canceled: ${fileName}`)
		this.name = 'CurseforgeDownloadCancelled'
	}
}
let manualQueue: Promise<unknown> = Promise.resolve()
const manualRequests = new Map<string, Promise<string>>()
export function setCurseforgeManualDownloadHandler(
	handler: NonNullable<typeof manualDownloadHandler>,
) {
	manualDownloadHandler = handler
}

export async function requestCurseforgeManualDownload(
	file: CurseforgeManualDownload,
): Promise<string> {
	if (!manualDownloadHandler)
		throw new Error(
			`CurseForge manual download required: ${file.fileName}${file.pageUrl ? ` — ${file.pageUrl}` : ''}`,
		)
	const key = `${file.projectId}/${file.fileId}/${file.sha1}`
	let pending = manualRequests.get(key)
	if (!pending) {
		pending = manualQueue
			.catch(() => {})
			.then(async () => {
				const path = await manualDownloadHandler!(file)
				if (!path) throw new CurseforgeDownloadCancelled(file.fileName)
				return path
			})
			.finally(() => manualRequests.delete(key))
		manualRequests.set(key, pending)
		manualQueue = pending
	}
	return pending
}

const LOADER_TYPE_BY_NAME = Object.fromEntries(
	Object.entries(LOADER_BY_TYPE).map(([type, name]) => [name, type]),
) as Record<string, string>

/** The newest file of `modId` that fits the instance, preferring releases. */
async function latestCompatibleFile(
	modId: number,
	gameVersion: string,
	loader: string,
): Promise<CfFile | null> {
	const body = await api<{ data: CfFile[] }>(`mods/${modId}/files`, {
		gameVersion,
		modLoaderType: LOADER_TYPE_BY_NAME[loader],
		pageSize: '50',
	})
	const files = body.data.sort((a, b) => Date.parse(b.fileDate) - Date.parse(a.fileDate))
	return files.find((file) => file.releaseType === 1) ?? files[0] ?? null
}

export interface CurseforgeInstallResult {
	projectId: string
	versionId: string
	dependencies: { projectId: string; versionId: string }[]
}

/**
 * Adds a CurseForge version to an instance, plus its required dependencies
 * (newest file compatible with the instance's game version and loader),
 * skipping files the instance already has.
 */
export async function installCurseforgeVersionToInstance(
	instanceId: string,
	versionId: string,
	contentType?: string,
): Promise<CurseforgeInstallResult> {
	const instance = await invoke<{ game_version: string; loader: string } | null>(
		'plugin:instance|instance_get',
		{ instanceId },
	)
	const existing = await invoke<Record<string, unknown>>('plugin:instance|instance_get_projects', {
		instanceId,
	})
	const existingNames = new Set(
		Object.keys(existing).map((path) =>
			path
				.split('/')
				.pop()
				?.replace(/\.disabled$/, ''),
		),
	)

	const { modId } = parseVersionId(versionId)
	const [primaryVersion] = await getCurseforgeVersions([versionId])
	const pending = [
		{ versionId, projectType: contentType, path: await downloadVersionFile(versionId) },
	]
	const result: CurseforgeInstallResult = {
		projectId: projectIdOf(modId),
		versionId,
		dependencies: [],
	}
	if (!instance) {
		await invoke('plugin:instance|instance_add_project_from_path', {
			instanceId,
			projectPath: pending[0].path,
			projectType: contentType === 'shader' ? 'shaderpack' : contentType,
		})
		return result
	}

	const visited = new Set([modId])
	const queue = [...primaryVersion.dependencies]
	while (queue.length > 0) {
		const dependency = queue.shift()!
		const dependencyModId = parseProjectId(dependency.project_id!)
		if (visited.has(dependencyModId)) continue
		visited.add(dependencyModId)

		const file = await latestCompatibleFile(dependencyModId, instance.game_version, instance.loader)
		if (!file)
			throw new Error(`No compatible CurseForge file for required dependency ${dependencyModId}`)
		queue.push(...toVersion(file, 'mod').dependencies)
		if (existingNames.has(file.fileName)) continue

		const dependencyVersionId = versionIdOf(dependencyModId, file.id)
		const dependencyType = projectTypeOf(await getMod(dependencyModId))
		pending.push({
			versionId: dependencyVersionId,
			projectType: dependencyType,
			path: await downloadVersionFile(dependencyVersionId),
		})
		existingNames.add(file.fileName)
		result.dependencies.push({
			projectId: projectIdOf(dependencyModId),
			versionId: dependencyVersionId,
		})
	}
	// All required downloads (including manual selections) succeed before changing the instance.
	for (const file of pending)
		await invoke('plugin:instance|instance_add_project_from_path', {
			instanceId,
			projectPath: file.path,
			projectType: file.projectType === 'shader' ? 'shaderpack' : file.projectType,
		})
	return result
}

/** A required modpack file that needs an official manual download. */
export interface CurseforgeSkippedFile {
	name: string
	pageUrl: string | null
	projectId: number
	fileId: number
}

/**
 * Downloads a CurseForge modpack version and converts it into an `.mrpack`
 * (see `theseus::curseforge_pack`), so it installs through the native
 * modpack installer like any other pack. Required restricted files are verified
 * manually before starting the install. Returns the `.mrpack` path.
 */
export async function downloadCurseforgeModpack(
	versionId: string,
	/** Omit to skip reporting left-out files (e.g. for a preview). */
	packName?: string,
): Promise<string> {
	const zipPath = await downloadVersionFile(versionId)
	return convertCurseforgeModpack(zipPath, packName)
}

/** Convert a local CurseForge export using the same installer as catalog downloads. */
export async function convertCurseforgeModpack(
	path: string,
	/** Omit while inspecting a preview to avoid showing duplicate notifications. */
	packName?: string,
): Promise<string> {
	const converted = await invoke<{ path: string; skipped: CurseforgeSkippedFile[] }>(
		'plugin:orbiont|orbiont_convert_curseforge_pack',
		{ path },
	)
	if (packName !== undefined && converted.skipped.length > 0) {
		if (!manualDownloadHandler) throw new Error('CurseForge manual download required for this pack')
		for (const file of converted.skipped)
			await downloadVersionFile(versionIdOf(file.projectId, file.fileId))
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
