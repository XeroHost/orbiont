import type { AbstractApiClient } from '../core/abstract-client'
import type { AbstractModule } from '../core/abstract-module'
import {
	LabrinthProjectsV2Module,
	LabrinthProjectsV3Module,
	LabrinthTagsV2Module,
	LabrinthUsersV2Module,
	LabrinthUsersV3Module,
} from './labrinth'
import { LauncherMetaManifestV0Module } from './launcher-meta/v0'
import { MclogsInsightsV1Module } from './mclogs/insights/v1'
import { MclogsLogsV1Module } from './mclogs/logs/v1'

type ModuleConstructor = new (client: AbstractApiClient) => AbstractModule

/**
 * To add a new module:
 * 1. Create your module class extending AbstractModule
 * 2. Add one line here: `<api>_<module>: YourModuleClass`
 *
 * TypeScript will automatically infer the client's field structure from this registry.
 *
 * TODO: Better way? Probably not
 */
export const MODULE_REGISTRY = {
	mclogs_insights_v1: MclogsInsightsV1Module,
	mclogs_logs_v1: MclogsLogsV1Module,
	launchermeta_manifest_v0: LauncherMetaManifestV0Module,
	labrinth_projects_v2: LabrinthProjectsV2Module,
	labrinth_projects_v3: LabrinthProjectsV3Module,
	labrinth_tags_v2: LabrinthTagsV2Module,
	labrinth_users_v2: LabrinthUsersV2Module,
	labrinth_users_v3: LabrinthUsersV3Module,
} as const satisfies Record<string, ModuleConstructor>

export type ModuleID = keyof typeof MODULE_REGISTRY

/**
 * Parse a module ID into [api, moduleName] tuple
 *
 * @param id - Module ID in format `<api>_<module>` (e.g., 'labrinth_projects_v2')
 * @returns Tuple of [api, moduleName] (e.g., ['labrinth', 'projects_v2'])
 * @throws Error if module ID doesn't match expected format
 */
export function parseModuleID(id: string): [string, string] {
	const parts = id.split('_')
	if (parts.length < 2) {
		throw new Error(
			`Invalid module ID "${id}". Expected format: <api>_<module> (e.g., "labrinth_projects_v2")`,
		)
	}
	const api = parts[0]
	const moduleName = parts.slice(1).join('_')
	return [api, moduleName]
}

/**
 * Build nested module structure from flat registry
 *
 * Transforms:
 * ```
 * { labrinth_projects_v2: Constructor, labrinth_users_v2: Constructor }
 * ```
 * Into:
 * ```
 * { labrinth: { projects_v2: Constructor, users_v2: Constructor } }
 * ```
 *
 * @returns Nested structure organized by API namespace
 */
export function buildModuleStructure(): Record<string, Record<string, ModuleConstructor>> {
	const structure: Record<string, Record<string, ModuleConstructor>> = {}

	for (const [id, constructor] of Object.entries(MODULE_REGISTRY)) {
		const [api, moduleName] = parseModuleID(id)

		if (!structure[api]) {
			structure[api] = {}
		}

		structure[api][moduleName] = constructor
	}

	return structure
}

/**
 * Extract API name from module ID
 * @example ParseAPI<'labrinth_projects_v2'> = 'labrinth'
 */
type ParseAPI<T extends string> = T extends `${infer API}_${string}` ? API : never

/**
 * Extract module name for a given API
 * @example ParseModule<'labrinth_projects_v2', 'labrinth'> = 'projects_v2'
 */
type ParseModule<T extends string, API extends string> = T extends `${API}_${infer Module}`
	? Module
	: never

/**
 * Group registry modules by API namespace
 *
 * Transforms flat registry into nested structure at the type level:
 * ```
 * { labrinth_projects_v2: ModuleClass } → { labrinth: { projects_v2: ModuleInstance } }
 * ```
 */
type GroupByAPI<Registry extends Record<string, ModuleConstructor>> = {
	[API in ParseAPI<keyof Registry & string>]: {
		[Module in ParseModule<keyof Registry & string, API>]: InstanceType<
			Registry[`${API}_${Module}`]
		>
	}
}

/**
 * Inferred client module structure
 **/
export type InferredClientModules = GroupByAPI<typeof MODULE_REGISTRY>
