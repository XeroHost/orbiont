import type { Labrinth } from '@orbiont/api-client'
import { useQuery } from '@tanstack/vue-query'
import { computed } from 'vue'

import { get_search_results_v3 } from '@/helpers/cache.js'
import { searchCurseforge } from '@/helpers/curseforge'
import { selectFeaturedModpacks } from '@/helpers/featured-modpacks'

export function useFeaturedModpacks() {
	const modrinth = useQuery({
		queryKey: ['sidebar', 'featured-modpacks', 'modrinth'],
		queryFn: async () => {
			const params = new URLSearchParams({
				facets: JSON.stringify([['project_type:modpack']]),
				index: 'relevance',
				limit: '12',
			})
			const response = (await get_search_results_v3(params.toString(), 'must_revalidate')) as {
				result: Labrinth.Search.v3.SearchResults
			} | null
			return response?.result.hits ?? []
		},
		staleTime: 5 * 60_000,
		gcTime: 0,
		retry: (attempt, error) => attempt < 1 && !String(error).includes('429'),
		refetchOnWindowFocus: false,
	})
	const curseforge = useQuery({
		queryKey: ['sidebar', 'featured-modpacks', 'curseforge'],
		queryFn: async () =>
			(
				await searchCurseforge({
					projectType: 'modpack',
					query: '',
					sort: 'relevance',
					limit: 12,
					page: 1,
					filters: [],
				})
			).hits,
		// Keep only active data; changing slides never fetches or stores a provider catalog on disk.
		staleTime: 0,
		gcTime: 0,
		retry: false,
		refetchOnWindowFocus: false,
	})
	return {
		items: computed(() =>
			selectFeaturedModpacks(modrinth.data.value ?? [], curseforge.data.value ?? []),
		),
		loading: computed(() => modrinth.isFetching.value || curseforge.isFetching.value),
		retry: () => Promise.allSettled([modrinth.refetch(), curseforge.refetch()]),
	}
}
