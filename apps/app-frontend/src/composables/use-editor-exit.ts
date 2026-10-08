import { getCurrentWindow } from '@tauri-apps/api/window'
import { onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'

import { createDraftExit } from '@/helpers/draft-exit'

/** Platform guards belong in the app, while the shared editor only exposes draft state. */
export function useEditorExit(isDirty: () => boolean, prompt: () => Promise<boolean>) {
	const exit = createDraftExit(isDirty, prompt)
	const removeRouteGuard = useRouter().beforeEach(() => exit.confirm())
	let removeCloseGuard: (() => void) | undefined
	let disposed = false
	let approvedClose = false
	let closePromptPending = false
	const beforeUnload = (event: BeforeUnloadEvent) => {
		if (approvedClose || !isDirty()) return
		event.preventDefault()
		event.returnValue = ''
	}
	onMounted(async () => {
		window.addEventListener('beforeunload', beforeUnload)
		if (!('__TAURI_INTERNALS__' in window)) return
		const currentWindow = getCurrentWindow()
		const unlisten = await currentWindow.onCloseRequested((event) => {
			if (approvedClose || !isDirty()) return
			event.preventDefault()
			if (closePromptPending) return
			closePromptPending = true
			void exit.confirm().then(async (approved) => {
				closePromptPending = false
				if (!approved || disposed) return
				approvedClose = true
				try {
					// Match Tauri's approved-close action without sending a second close request.
					await currentWindow.destroy()
				} catch {
					approvedClose = false
					console.error('Unable to close the editor window')
				}
			})
		})
		if (disposed) unlisten()
		else removeCloseGuard = unlisten
	})
	onUnmounted(() => {
		disposed = true
		removeRouteGuard()
		removeCloseGuard?.()
		window.removeEventListener('beforeunload', beforeUnload)
	})
	return exit.confirm
}
