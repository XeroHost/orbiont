import { computed, ref } from 'vue'

import { injectAppBackup } from '#ui/providers'

/** Offers a local instance backup before a risky content change. */
export function useInlineBackup() {
	const appBackup = injectAppBackup(null)
	const isBackingUp = ref(false)
	const backupFailed = ref(false)
	const backupComplete = ref(false)

	return {
		available: appBackup !== null,
		isBackingUp,
		backupFailed,
		backupComplete,
		externalBackupInProgress: computed(() => false),
		startBackup: async () => {
			if (!appBackup) return
			isBackingUp.value = true
			backupFailed.value = false
			backupComplete.value = false
			try {
				await appBackup.createBackup()
				backupComplete.value = true
			} catch {
				backupFailed.value = true
			} finally {
				isBackingUp.value = false
			}
		},
		cancelBackup: async () => {},
	}
}
