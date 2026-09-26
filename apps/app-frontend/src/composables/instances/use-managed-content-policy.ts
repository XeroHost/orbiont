import type { ContentItem } from '@modrinth/ui'
import { computed, type Ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

/** What can be changed in an instance's content: everything, unless it's quarantined. */
export function useManagedContentPolicy(instance: Ref<GameInstance>) {
	const isQuarantined = computed(() => instance.value.quarantined)

	function canMutateContent(_item: ContentItem) {
		return !isQuarantined.value
	}

	function canUpdateContent(item: ContentItem) {
		return (
			canMutateContent(item) && !!item.file_path && !!item.has_update && !!item.update_version_id
		)
	}

	return {
		isQuarantined,
		canMutateContent,
		canUpdateContent,
	}
}
