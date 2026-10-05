import assert from 'node:assert/strict'
import { resolve } from 'node:path'
import { after, before, test } from 'node:test'

import { createServer } from 'vite'

let server, AppNotificationManager
before(async () => {
	server = await createServer({
		configFile: false,
		root: resolve(import.meta.dirname, '..'),
		server: { middlewareMode: true, hmr: false },
		optimizeDeps: { noDiscovery: true, include: [] },
		appType: 'custom',
		plugins: [
			{
				name: 'notification-context-boundary',
				enforce: 'pre',
				resolveId(id, importer) {
					if (id === '@orbiont/ui') return '\0notification-base'
					if (
						id === '.' &&
						importer?.replaceAll('\\', '/').endsWith('/providers/web-notifications.ts')
					)
						return '\0context'
				},
				load(id) {
					if (id === '\0notification-base')
						return `export { AbstractWebNotificationManager } from ${JSON.stringify(resolve(import.meta.dirname, '../../../packages/ui/src/providers/web-notifications.ts').replaceAll('\\', '/'))}`
					if (id === '\0context') return 'export const createContext = () => [() => {}, () => {}]'
				},
			},
		],
	})
	;({ AppNotificationManager } = await server.ssrLoadModule('/src/providers/app-notifications.ts'))
})
after(async () => {
	await server?.close()
})

test('429s for different paths share one localized notification', () => {
	const manager = new AppNotificationManager(() => ({
		title: 'Límite de CurseForge',
		text: 'Espera y vuelve a intentarlo.',
	}))
	try {
		manager.handleError(new Error('CurseForge request to categories failed: 429 Too Many Requests'))
		manager.handleError(
			new Error('CurseForge request to mods/10/files failed: 429 Too Many Requests'),
		)
		assert.equal(manager.getNotifications().length, 1)
		assert.equal(manager.getNotifications()[0].count, 2)
		assert.equal(manager.getNotifications()[0].title, 'Límite de CurseForge')
		assert.equal(manager.getNotifications()[0].type, 'warning')
	} finally {
		manager.clearAllNotifications()
	}
})

test('distribution restrictions remain separate from rate-limit warnings', () => {
	const manager = new AppNotificationManager(() => ({ title: 'Limit', text: 'Wait' }))
	try {
		manager.handleError(new Error('The author only allows downloading from CurseForge’s website'))
		manager.handleError(new Error('CurseForge request failed: 429 Too Many Requests'))
		assert.equal(manager.getNotifications().length, 2)
		assert.equal(manager.getNotifications()[0].type, 'error')
	} finally {
		manager.clearAllNotifications()
	}
})
