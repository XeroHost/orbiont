/** Native selected-update planning currently resolves only native cache IDs. */
export function supportsNativeSelectedUpdate(item: {
	project?: { id: string } | null
	version?: { id: string } | null
	update_version_id?: string | null
}) {
	return (
		!!item.project?.id &&
		![item.project.id, item.version?.id, item.update_version_id].some((id) => id?.startsWith('cf-'))
	)
}
