import { invoke } from '@tauri-apps/api/core'

export type CatalogSource = 'xerohost' | 'modrinth' | 'curseforge'

export interface Modpack {
	id: string
	name: string
	description: string
	icon: string
	gameVersion: string
	loader: string
	downloadUrl: string
	hash: string
	source: CatalogSource
}

export interface OrbiontServer {
	id: string
	name: string
	host: string
	port: number
	icon: string
	tags: string[]
	featured: boolean
	modpackId: string | null
}

/** One normalized hit from GET /v1/search, regardless of which source produced it. */
export interface SearchResult {
	id: string
	source: CatalogSource
	name: string
	description: string
	icon: string | null
	downloads: number | null
	gameVersions: string[]
	loaders: string[]
	/**
	 * Null when the source doesn't expose a direct download for this result
	 * (e.g. a CurseForge file whose author disabled third-party distribution,
	 * or a Modrinth project search hit, which is project- not file-level).
	 * Don't hide the result — show it with a link to `pageUrl` instead.
	 */
	downloadUrl: string | null
	/** CurseForge-specific; null for other sources. */
	allowModDistribution: boolean | null
	pageUrl: string | null
}

export interface SearchResponse {
	results: SearchResult[]
	index: number
	pageSize: number
	totalCount: number | null
	errors?: Array<{ source: CatalogSource; message: string }>
}

/** One of CurseForge's own categories (e.g. "Tech", "Magic") for a project type. */
export interface CurseforgeCategory {
	id: number
	name: string
	slug: string
	iconUrl: string | null
}

export interface CurseforgeScreenshot {
	url: string
	thumbnailUrl: string
	title: string
}

/** Full detail for a single CurseForge mod/modpack, for a search result's detail view. */
export interface CurseforgeModDetail {
	id: string
	source: CatalogSource
	name: string
	summary: string
	description: string
	icon: string | null
	downloads: number | null
	gameVersions: string[]
	categories: string[]
	authors: string[]
	screenshots: CurseforgeScreenshot[]
	downloadUrl: string | null
	allowModDistribution: boolean | null
	pageUrl: string | null
}

export async function getModpacks(): Promise<Modpack[]> {
	return await invoke('plugin:orbiont|orbiont_get_modpacks')
}

export async function getServers(): Promise<OrbiontServer[]> {
	return await invoke('plugin:orbiont|orbiont_get_servers')
}

export type CurseforgeProjectType = 'modpack' | 'mod' | 'resourcepack' | 'datapack' | 'shader'
export type CurseforgeModLoader = 'forge' | 'fabric' | 'quilt' | 'neoforge'

export async function searchCatalog(options: {
	query?: string
	source?: CatalogSource | 'all'
	gameVersion?: string
	/** Defaults to "modpack" on the backend; CurseForge also supports the other project types. */
	projectType?: CurseforgeProjectType
	/** CurseForge-only; ignored by other sources. */
	categoryId?: number
	/** CurseForge-only; ignored by other sources. */
	modLoaderType?: CurseforgeModLoader
	index?: number
	pageSize?: number
}): Promise<SearchResponse> {
	return await invoke('plugin:orbiont|orbiont_search', {
		query: options.query,
		source: options.source,
		gameVersion: options.gameVersion,
		projectType: options.projectType,
		categoryId: options.categoryId,
		modLoaderType: options.modLoaderType,
		index: options.index,
		pageSize: options.pageSize,
	})
}

/** CurseForge's own category list for a project type. */
export async function getCurseforgeCategories(
	projectType: CurseforgeProjectType,
): Promise<CurseforgeCategory[]> {
	return await invoke('plugin:orbiont|orbiont_get_curseforge_categories', { projectType })
}

/**
 * Full detail for a single CurseForge mod/modpack, for a search result's
 * detail view. `modId` is the numeric CurseForge id — strip the
 * "curseforge:" prefix from a `SearchResult.id` before calling this.
 */
export async function getCurseforgeModDetail(modId: string): Promise<CurseforgeModDetail> {
	return await invoke('plugin:orbiont|orbiont_get_curseforge_mod_detail', { modId })
}

/** Downloads a catalog modpack's .mrpack to a local cache path for installing. */
export async function downloadModpack(modpack: Modpack): Promise<string> {
	return await invoke('plugin:orbiont|orbiont_download_modpack', { modpack })
}

/**
 * Downloads a search result's file (e.g. a CurseForge `downloadUrl`) to a
 * local cache path for installing. Only call this when `downloadUrl` isn't
 * null — see `SearchResult.downloadUrl`.
 */
export async function downloadSearchResult(url: string, fileNameHint: string): Promise<string> {
	return await invoke('plugin:orbiont|orbiont_download_search_result', {
		url,
		fileNameHint,
	})
}

/**
 * Saves a modpack file the user dropped onto the launcher to a local cache
 * path, for search results where the author disabled third-party
 * distribution and the user downloaded the file manually.
 */
export async function saveDroppedFile(bytes: Uint8Array, fileNameHint: string): Promise<string> {
	return await invoke('plugin:orbiont|orbiont_save_dropped_file', {
		bytes: Array.from(bytes),
		fileNameHint,
	})
}
