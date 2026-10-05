export interface FeaturedSearchHit {
	project_id: string
	name: string
	slug: string | null
	summary: string
	icon_url?: string | null
	featured_gallery?: string | null
	gallery?: string[]
	page_url?: string | null
}

export interface FeaturedModpack {
	projectId: string
	provider: 'modrinth' | 'curseforge'
	name: string
	summary: string
	imageUrl: string | null
	iconUrl: string | null
}

function normalized(value: string) {
	return value
		.normalize('NFKD')
		.replace(/\p{M}/gu, '')
		.toLowerCase()
		.replace(/[^\p{L}\p{N}]/gu, '')
}

function projectName(name: string) {
	// Some publishers append the pack's acronym ("All the Mods 10 - ATM10").
	// Remove that exact acronym, without merging numbered editions or arbitrary subtitles.
	const suffix = name.match(/^(.*?)\s+[-–—]\s+([a-zA-Z0-9]+)$/)
	if (suffix) {
		const words = suffix[1].match(/[a-zA-Z]+|\d+/g) ?? []
		const acronym = words.map((word) => (/^\d+$/.test(word) ? word : word[0])).join('')
		if (normalized(acronym) === normalized(suffix[2])) return normalized(suffix[1])
	}
	return normalized(name)
}

function identities(hit: FeaturedSearchHit) {
	const keys = [projectName(hit.name)]
	// The CurseForge adapter exposes cf-<id> as its slug; recover its real slug from its page.
	if (hit.slug && !/^cf-\d+$/.test(hit.slug)) keys.push(normalized(hit.slug))
	if (hit.page_url) {
		try {
			const url = new URL(hit.page_url)
			if (url.hostname === 'www.curseforge.com' || url.hostname === 'curseforge.com') {
				const slug = url.pathname.match(/^\/minecraft\/modpacks\/([^/]+)\/?$/)?.[1]
				if (slug) keys.push(normalized(decodeURIComponent(slug)))
			}
		} catch {
			/* A missing or invalid provider URL cannot identify a duplicate. */
		}
	}
	return keys.filter(Boolean)
}

function imageUrl(url: string | null | undefined): string | null {
	return url && /^https?:\/\//i.test(url) ? url : null
}

/** Preserve each provider's ranking; Modrinth wins duplicates before filling CurseForge's quota. */
export function selectFeaturedModpacks(
	modrinth: FeaturedSearchHit[],
	curseforge: FeaturedSearchHit[],
): FeaturedModpack[] {
	const seen = new Set<string>()
	const selected: FeaturedModpack[][] = []
	for (const [provider, hits] of [
		['modrinth', modrinth],
		['curseforge', curseforge],
	] as const) {
		const items: FeaturedModpack[] = []
		for (const hit of hits) {
			if (!hit.project_id || !hit.name) continue
			const keys = identities(hit)
			const id = `${provider}:${hit.project_id}`
			if (seen.has(id) || keys.some((key) => seen.has(key))) continue
			seen.add(id)
			keys.forEach((key) => seen.add(key))
			const icon = imageUrl(hit.icon_url)
			items.push({
				projectId: hit.project_id,
				provider,
				name: hit.name,
				summary: hit.summary,
				imageUrl: imageUrl(hit.featured_gallery) ?? imageUrl(hit.gallery?.[0]) ?? icon,
				iconUrl: icon,
			})
			if (items.length === 3) break
		}
		selected.push(items)
	}
	return Array.from({ length: 3 }, (_, index) => [selected[0][index], selected[1][index]])
		.flat()
		.filter((item): item is FeaturedModpack => !!item)
}
