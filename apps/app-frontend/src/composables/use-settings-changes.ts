import { inject, onBeforeUnmount, onMounted } from 'vue'

import {
	appSettingsModalContextKey,
	type UnsavedChangesController,
} from '@/providers/app-settings-modal'

export function useSettingsChanges(key: string, controller: UnsavedChangesController) {
	const modal = inject(appSettingsModalContextKey, null)
	onMounted(() => modal?.registerUnsavedChangesController(controller, key))
	onBeforeUnmount(() => modal?.registerUnsavedChangesController(null, key))
}
