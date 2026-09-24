import { useQuery } from '@tanstack/vue-query'
import { computed } from 'vue'

import { getModpacks, getServers } from '@/helpers/orbiont'

/** XeroHost catalog: modpacks and servers (featured first). */
export function useOrbiontCatalog() {
	const modpacksQuery = useQuery({
		queryKey: ['orbiont', 'modpacks'],
		queryFn: getModpacks,
		staleTime: 60_000,
	})
	const serversQuery = useQuery({
		queryKey: ['orbiont', 'servers'],
		queryFn: getServers,
		staleTime: 60_000,
	})

	const modpacks = computed(() => modpacksQuery.data.value ?? [])
	const servers = computed(() =>
		(serversQuery.data.value ?? []).slice().sort((a, b) => Number(b.featured) - Number(a.featured)),
	)
	const loading = computed(() => serversQuery.isPending.value || modpacksQuery.isPending.value)
	const error = computed(() => serversQuery.error.value)

	return { modpacks, servers, loading, error, refetch: () => serversQuery.refetch() }
}
