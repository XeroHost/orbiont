import { invoke } from '@tauri-apps/api/core'

// CurseForge ids (`cf-…`) resolve through the CurseForge data adapter into the
// same shapes the Modrinth cache returns, so every page that reads projects,
// versions or teams works for both providers without provider-specific UI.
import {
	getCurseforgeProject,
	getCurseforgeProjectV3,
	getCurseforgeProjectVersions,
	getCurseforgeTeam,
	getCurseforgeVersion,
	getCurseforgeVersions,
	isCurseforgeId,
} from '@/helpers/curseforge'

async function withCurseforge(ids, getCurseforge, getNative) {
	const curseforgeIds = ids.filter(isCurseforgeId)
	const nativeIds = ids.filter((id) => !isCurseforgeId(id))
	const [curseforge, native] = await Promise.all([
		curseforgeIds.length ? getCurseforge(curseforgeIds) : [],
		nativeIds.length ? getNative(nativeIds) : [],
	])
	return [...(native ?? []), ...curseforge]
}

export async function get_project(id, cacheBehaviour) {
	if (isCurseforgeId(id)) return await getCurseforgeProject(id)
	return await invoke('plugin:cache|get_project', { id, cacheBehaviour })
}

export async function get_project_many(ids, cacheBehaviour) {
	return await withCurseforge(
		ids,
		(curseforgeIds) => Promise.all(curseforgeIds.map(getCurseforgeProject)),
		(nativeIds) => invoke('plugin:cache|get_project_many', { ids: nativeIds, cacheBehaviour }),
	)
}

export async function get_project_v3(id, cacheBehaviour) {
	if (isCurseforgeId(id)) return await getCurseforgeProjectV3(id)
	return await invoke('plugin:cache|get_project_v3', { id, cacheBehaviour })
}

export async function get_project_v3_many(ids, cacheBehaviour) {
	return await withCurseforge(
		ids,
		(curseforgeIds) => Promise.all(curseforgeIds.map(getCurseforgeProjectV3)),
		(nativeIds) => invoke('plugin:cache|get_project_v3_many', { ids: nativeIds, cacheBehaviour }),
	)
}

export async function get_version(id, cacheBehaviour) {
	if (isCurseforgeId(id)) return await getCurseforgeVersion(id)
	return await invoke('plugin:cache|get_version', { id, cacheBehaviour })
}

export async function get_version_many(ids, cacheBehaviour) {
	return await withCurseforge(ids, getCurseforgeVersions, (nativeIds) =>
		invoke('plugin:cache|get_version_many', { ids: nativeIds, cacheBehaviour }),
	)
}

export async function get_user(id, cacheBehaviour) {
	return await invoke('plugin:cache|get_user', { id, cacheBehaviour })
}

export async function get_user_many(ids, cacheBehaviour) {
	return await invoke('plugin:cache|get_user_many', { ids, cacheBehaviour })
}

export async function get_team(id, cacheBehaviour) {
	if (isCurseforgeId(id)) return await getCurseforgeTeam(id)
	return await invoke('plugin:cache|get_team', { id, cacheBehaviour })
}

export async function get_team_many(ids, cacheBehaviour) {
	return await invoke('plugin:cache|get_team_many', { ids, cacheBehaviour })
}

export async function get_organization(id, cacheBehaviour) {
	return await invoke('plugin:cache|get_organization', { id, cacheBehaviour })
}

export async function get_organization_many(ids, cacheBehaviour) {
	return await invoke('plugin:cache|get_organization_many', { ids, cacheBehaviour })
}

export async function get_search_results(id, cacheBehaviour) {
	return await invoke('plugin:cache|get_search_results', { id, cacheBehaviour })
}

export async function get_search_results_many(ids, cacheBehaviour) {
	return await invoke('plugin:cache|get_search_results_many', { ids, cacheBehaviour })
}

export async function get_search_results_v3(id, cacheBehaviour) {
	return await invoke('plugin:cache|get_search_results_v3', { id, cacheBehaviour })
}

export async function get_search_results_v3_many(ids, cacheBehaviour) {
	return await invoke('plugin:cache|get_search_results_v3_many', { ids, cacheBehaviour })
}

export async function purge_cache_types(cacheTypes) {
	return await invoke('plugin:cache|purge_cache_types', { cacheTypes })
}

/**
 * Get versions for a project (without changelogs for fast loading).
 * Uses the cache system - versions are cached for 30 minutes.
 * @param {string} projectId - The project ID
 * @param {string} [cacheBehaviour] - Cache behaviour ('must_revalidate', etc.)
 * @returns {Promise<Array|null>} Array of version objects (without changelogs) or null
 */
export async function get_project_versions(projectId, cacheBehaviour) {
	if (isCurseforgeId(projectId)) {
		return await getCurseforgeProjectVersions(projectId)
	}
	return await invoke('plugin:cache|get_project_versions', {
		projectId,
		cacheBehaviour,
	})
}
