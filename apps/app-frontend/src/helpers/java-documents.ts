import { invoke } from '@tauri-apps/api/core'

/** Each revision is tied to both instance and path; failed writes never advance it. */
export function createJavaDocuments(instanceId: () => string) {
	const revisions = new Map<string, string>()
	return {
		clear: () => revisions.clear(),
		async read(path: string): Promise<string> {
			const id = instanceId()
			const document = await invoke<{ content: string; revision: string }>(
				'plugin:files|file_read_document',
				{ instanceId: id, path },
			)
			revisions.set(`${id}/${path}`, document.revision)
			return document.content
		},
		async write(path: string, content: string) {
			const id = instanceId()
			const key = `${id}/${path}`
			const expectedRevision = revisions.get(key)
			if (!expectedRevision) throw new Error('Document has not been read')
			const result = await invoke<{ revision: string }>('plugin:files|file_write_document', {
				instanceId: id,
				path,
				content,
				expectedRevision,
			})
			revisions.set(key, result.revision)
		},
	}
}
