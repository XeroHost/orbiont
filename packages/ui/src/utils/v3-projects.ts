import type { Labrinth } from '@orbiont/api-client'
import { getPrimaryProjectType } from '@orbiont/utils'

import { defineMessage } from '../composables/i18n'
import { normalizeProjectType } from './common-messages'
import { sortProjectTypes } from './project-types'

export type ProjectLinkMode = 'website' | 'app'

export const SERVER_REGIONS = {
	us_east: defineMessage({ id: 'project.server.region.us_east', defaultMessage: 'US East' }),
	us_west: defineMessage({ id: 'project.server.region.us_west', defaultMessage: 'US West' }),
	europe: defineMessage({ id: 'project.server.region.europe', defaultMessage: 'Europe' }),
	asia: defineMessage({ id: 'project.server.region.asia', defaultMessage: 'Asia' }),
	australia: defineMessage({ id: 'project.server.region.australia', defaultMessage: 'Australia' }),
	south_america: defineMessage({
		id: 'project.server.region.south_america',
		defaultMessage: 'South America',
	}),
	middle_east: defineMessage({
		id: 'project.server.region.middle_east',
		defaultMessage: 'Middle East',
	}),
	russia: defineMessage({ id: 'project.server.region.russia', defaultMessage: 'Russia' }),
}

export type ServerModpackContent = {
	name: string
	icon?: string
	onclick?: () => void
	showCustomModpackTooltip?: boolean
}

export function isProjectServer(project: Labrinth.Projects.v3.Project): boolean {
	return project.minecraft_server != null
}

export function getProjectCardTags(project: Labrinth.Projects.v3.Project): string[] {
	if (isProjectServer(project)) {
		return project.categories
	}
	return [...project.categories, ...project.loaders, ...(project.mrpack_loaders ?? [])]
}

export function getProjectCardAllTags(project: Labrinth.Projects.v3.Project): string[] {
	return [...getProjectCardTags(project), ...project.additional_categories]
}

export function getProjectPagePath(
	project: Labrinth.Projects.v3.Project,
	linkMode: ProjectLinkMode = 'website',
): string {
	if (linkMode === 'app') {
		return `/project/${project.id}`
	}
	if (isProjectServer(project)) {
		return `/server/${project.slug || project.id}`
	}
	return `/${getPrimaryProjectType(project)}/${project.slug || project.id}`
}

export function getServerModpackContent(
	project: Labrinth.Projects.v3.Project,
	onNavigate?: (projectId: string) => void,
): ServerModpackContent | undefined {
	const content = project.minecraft_java_server?.content
	if (content?.kind !== 'modpack') {
		return undefined
	}

	const { project_name, project_icon, project_id } = content
	if (!project_name) {
		return undefined
	}

	return {
		name: project_name,
		icon: project_icon,
		onclick:
			project_id && project_id !== project.id && onNavigate
				? () => onNavigate(project_id)
				: undefined,
		showCustomModpackTooltip: project_id === project.id,
	}
}

export function catalogProjectTypes(projects: Labrinth.Projects.v3.Project[]): string[] {
	const types = new Set(projects.map((project) => getPrimaryProjectType(project)))
	types.delete('project')
	return sortProjectTypes(types)
}

export function parseProjectTypeRouteParam(param: unknown): string | null {
	if (param == null) {
		return null
	}
	const type = Array.isArray(param) ? param[0] : param
	if (typeof type !== 'string' || type.length === 0) {
		return null
	}
	if (type === 'collection' || type === 'collections') {
		return 'collection'
	}
	const singular = type.endsWith('s') ? type.slice(0, -1) : type
	return normalizeProjectType(singular)
}

export function filterProjectsByType(
	projects: Labrinth.Projects.v3.Project[],
	projectType: string | null,
): Labrinth.Projects.v3.Project[] {
	if (!projectType) {
		return projects
	}
	return projects.filter((project) => getPrimaryProjectType(project) === projectType)
}
