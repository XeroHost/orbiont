import assert from 'node:assert/strict'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { after, before, test } from 'node:test'

import { createServer } from 'vite'

let server
let catalog
let invoke
const previousWindow = globalThis.window
before(async () => {
	globalThis.window = { __TAURI_INTERNALS__: { invoke: (...args) => invoke(...args) } }
	server = await createServer({
		configFile: false,
		root: resolve(import.meta.dirname, '..'),
		cacheDir: join(tmpdir(), 'orbiont-skin-catalog-tests'),
		resolve: { alias: { '@': resolve(import.meta.dirname, '../src') } },
		optimizeDeps: { noDiscovery: true, include: [] },
		server: { middlewareMode: true, hmr: false },
		appType: 'custom',
	})
	catalog = await server.ssrLoadModule('/src/helpers/skin-catalog.ts')
})
after(async () => {
	await server?.close()
	if (previousWindow === undefined) delete globalThis.window
	else globalThis.window = previousWindow
})

test('queries the native provider boundary once with the selected search filters', async () => {
	const calls = []
	invoke = async (command, args) => {
		calls.push({ command, args })
		return { provider: 'mcstat', skins: [], page: 2, next: null }
	}
	const result = await catalog.getSkinCatalog('mcstat', {
		page: 2,
		search: 'knight',
		model: 'slim',
		tag: undefined,
	})
	assert.equal(result.page, 2)
	assert.deepEqual(calls, [
		{
			command: 'plugin:minecraft-skins|get_skin_catalog',
			args: {
				provider: 'mcstat',
				query: [
					['page', '2'],
					['search', 'knight'],
					['model', 'slim'],
				],
			},
		},
	])
})

test('MineSkin forwards gallery text with its cursor and retains provider matches with other names', async () => {
	const calls = []
	const skin = {
		id: 'gallery-result',
		name: 'Different visible name',
		texture: 'https://textures.minecraft.net/texture/example',
		model: 'unknown',
		tags: [],
	}
	invoke = async (_, args) => {
		calls.push(args)
		return { provider: 'mineskin', skins: [skin], page: 1, next: 'next-cursor' }
	}
	const page = await catalog.getSkinCatalog('mineskin', {
		search: 'Raimond',
		after: 'previous-cursor',
	})
	assert.deepEqual(calls, [
		{
			provider: 'mineskin',
			query: [
				['search', 'Raimond'],
				['after', 'previous-cursor'],
			],
		},
	])
	assert.deepEqual(page.skins, [skin])
	assert.equal(page.next, 'next-cursor')
})

test('the same texture is downloaded and normalized only once for preview and saving', async () => {
	const calls = []
	const png = [137, 80, 78, 71]
	invoke = async (command) => {
		calls.push(command)
		return png
	}
	const item = {
		id: 'knight',
		name: 'Knight',
		texture: 'https://mcstat.org/media/skins/published/knight.png',
		model: 'slim',
		tags: [],
	}
	const skin = catalog.catalogSkin(item, 'mcstat')
	const [texture, prepared] = await Promise.all([
		catalog.getCatalogTexture(item.texture),
		catalog.prepareCatalogSkin(skin),
	])
	assert.deepEqual([...texture], png)
	assert.equal(prepared.skin.variant, 'SLIM')
	assert.equal(prepared.skin.texture, 'data:image/png;base64,iVBORw==')
	assert.equal(prepared.skin.source, 'custom_external')
	assert.equal(prepared.skin.section, 'catalog')
	assert.equal(prepared.skin.texture_key, 'catalog:mcstat:knight')
	assert.deepEqual(calls, [
		'plugin:minecraft-skins|get_catalog_skin_texture',
		'plugin:minecraft-skins|normalize_skin_texture',
	])
})

test('a failed texture download can be retried rather than poisoning the preview cache', async () => {
	let calls = 0
	const url = 'https://textures.minecraft.net/texture/retry-test'
	invoke = async (command) => {
		if (command.includes('get_catalog_skin_texture') && ++calls === 1) throw new Error('offline')
		return [1, 2, 3]
	}
	await assert.rejects(catalog.getCatalogTexture(url), /offline/)
	assert.deepEqual([...(await catalog.getCatalogTexture(url))], [1, 2, 3])
	assert.equal(calls, 2)
})

test('Retry-After stops repeated native calls while another provider remains available', async () => {
	const calls = []
	invoke = async (_, args) => {
		calls.push(args.provider)
		if (args.provider === 'mcstat') throw new Error('Skin catalog mcstat: 429; Retry-After: 120')
		return { provider: 'mineskin', skins: [], page: 1, next: null }
	}
	await assert.rejects(catalog.getSkinCatalog('mcstat', {}), /429/)
	await assert.rejects(catalog.getSkinCatalog('mcstat', { page: 2 }), /429/)
	await catalog.getSkinCatalog('mineskin', {})
	assert.deepEqual(calls, ['mcstat', 'mineskin'])
})
