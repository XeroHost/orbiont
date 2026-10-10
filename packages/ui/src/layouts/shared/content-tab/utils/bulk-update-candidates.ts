export function partitionBulkUpdateCandidates<T>(items: T[], canUpdate?: (item: T) => boolean) {
	return {
		supported: items.filter((item) => canUpdate?.(item) !== false),
		unsupported: items.filter((item) => canUpdate?.(item) === false),
	}
}
