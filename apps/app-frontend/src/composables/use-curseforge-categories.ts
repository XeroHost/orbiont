import { useQuery } from '@tanstack/vue-query'
import type { Ref } from 'vue'
import { computed } from 'vue'

import type { CurseforgeProjectType, MinecraftEdition } from '@/helpers/curseforge'
import { getCurseforgeCategoryTags } from '@/helpers/curseforge'

export function useCurseforgeCategories(
	projectType: Ref<string>,
	enabled: Ref<boolean>,
	edition: MinecraftEdition = 'java',
) {
	return useQuery(
		computed(() => {
			// Capture the type for this request; navigation can change the ref before it completes.
			const type = projectType.value as CurseforgeProjectType
			return {
				queryKey: ['curseforge', 'categories', type, edition] as const,
				queryFn: () => getCurseforgeCategoryTags(type, edition),
				enabled: enabled.value,
				// Keep only active query data. The helper coalesces concurrent requests;
				// do not retain a catalog cache after leaving the tab.
				staleTime: 0,
				gcTime: 0,
				retry: (attempt: number, error: unknown) => attempt < 2 && !String(error).includes('429'),
				retryDelay: (attempt: number) => 1000 * 2 ** attempt,
				refetchOnWindowFocus: false,
			}
		}),
	)
}
