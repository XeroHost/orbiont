import type { LocalCopy, LocalStorageEntry } from './local-management'

export interface RetentionPolicy {
	days: number
	keep: number
}

export function validRetentionPolicy(policy: RetentionPolicy): boolean {
	return (
		Number.isInteger(policy.days) &&
		policy.days >= 1 &&
		policy.days <= 3650 &&
		Number.isInteger(policy.keep) &&
		policy.keep >= 1 &&
		policy.keep <= 1000
	)
}

// Selection only: callers must still show and confirm permanent removal.
// Keep the newest copies per destination, not merely per installation.
export function retentionSelection(
	entries: LocalStorageEntry[],
	copies: LocalCopy[],
	policy: RetentionPolicy,
	now = Date.now() / 1000,
): Set<string> {
	if (!validRetentionPolicy(policy) || !Number.isFinite(now)) return new Set()
	const groups = new Map<string, LocalCopy[]>()
	const eligible = new Set(
		entries
			.filter((entry) => entry.category === 'recovery' && entry.removable && !entry.limited)
			.map((entry) => `${entry.rootId}/${entry.id}`),
	)
	for (const copy of copies) {
		if (!Number.isFinite(copy.created) || copy.created <= 0 || copy.limited) continue
		const group = JSON.stringify([copy.rootId, copy.path, copy.source])
		const bucket = groups.get(group) ?? []
		bucket.push(copy)
		groups.set(group, bucket)
	}
	const selected = new Set<string>()
	const cutoff = now - policy.days * 86400
	for (const group of groups.values()) {
		group.sort((a, b) => b.created - a.created || a.id.localeCompare(b.id))
		for (const copy of group.slice(policy.keep)) {
			const id = `${copy.rootId}/${copy.id}`
			if (
				eligible.has(id) &&
				copy.created < cutoff &&
				['available', 'applied', 'restored', 'rolled_back'].includes(copy.state)
			) {
				selected.add(id)
			}
		}
	}
	return selected
}
