import {
	type ImageViewerEditorData,
	type ImageViewerEditorSource,
	provideImageViewerEditor,
} from '@orbiont/ui'
import { readFile } from '@tauri-apps/plugin-fs'
import { fetch as nativeFetch } from '@tauri-apps/plugin-http'

export function setupImageViewerEditorProvider() {
	provideImageViewerEditor({
		async loadEditorData(source: ImageViewerEditorSource): Promise<ImageViewerEditorData> {
			return {
				source: new Blob([await readFile(source.path)], {
					type: /\.jpe?g$/i.test(source.path) ? 'image/jpeg' : 'image/png',
				}),
			}
		},
		async loadRemoteData(src: string): Promise<ImageViewerEditorData> {
			const url = new URL(src)
			// CurseForge's image CDN permits <img> display but not browser CORS fetches.
			const fetchImage =
				url.protocol === 'https:' && url.hostname === 'media.forgecdn.net' ? nativeFetch : fetch
			const response = await fetchImage(src)
			if (!response.ok) throw new Error(`Could not load image: ${response.statusText}`)
			return { source: await response.blob() }
		},
	})
}
