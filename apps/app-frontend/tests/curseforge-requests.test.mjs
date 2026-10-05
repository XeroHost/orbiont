import assert from 'node:assert/strict'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { after, before, test } from 'node:test'

import { createServer } from 'vite'

let server, adapter, invoke, catalog
const previousWindow = globalThis.window
const file = {
	id: 100,
	modId: 10,
	displayName: 'Release',
	fileName: 'mod.jar',
	fileDate: '2026-01-01',
	gameVersions: ['1.21.1', 'Fabric'],
	hashes: [],
	dependencies: [],
	downloadUrl: 'https://edge.forgecdn.net/mod.jar',
	releaseType: 1,
}
const mod = {
	id: 10,
	classId: 6,
	name: 'Mod',
	summary: '',
	links: {},
	authors: [],
	categories: [],
	screenshots: [],
	latestFiles: [file],
	latestFilesIndexes: [{ gameVersion: '1.21.1', fileId: 100, modLoader: 4 }],
}
before(async () => {
	globalThis.window = { __TAURI_INTERNALS__: { invoke: (...args) => invoke(...args) } }
	server = await createServer({
		configFile: false,
		root: resolve(import.meta.dirname, '..'),
		cacheDir: join(tmpdir(), 'orbiont-request-tests-vite'),
		resolve: { alias: { '@': resolve(import.meta.dirname, '../src') } },
		optimizeDeps: { noDiscovery: true, include: [] },
		server: { middlewareMode: true, hmr: false },
		appType: 'custom',
		plugins: [
			{
				name: 'icons-stub',
				enforce: 'pre',
				resolveId: (id) => (id === '@orbiont/assets' ? '\0icons' : undefined),
				load: (id) =>
					id === '\0icons' ? 'export const registerCategoryIconAliases = () => {}' : undefined,
			},
		],
	})
	adapter = await server.ssrLoadModule('/src/helpers/curseforge.ts')
	catalog = await server.ssrLoadModule('/src/helpers/bedrock-catalog.ts')
})
after(async () => {
	await server?.close()
	if (previousWindow === undefined) delete globalThis.window
	else globalThis.window = previousWindow
})

test('manual cancellation is distinguishable and allows retrying the same file', async () => {
	const restricted = { projectId: 10, fileId: 100, fileName: 'world.zip', sha1: 'a'.repeat(40) }
	try {
		adapter.setCurseforgeManualDownloadHandler(async () => null)
		await assert.rejects(adapter.requestCurseforgeManualDownload(restricted), (error) => {
			assert.ok(error instanceof adapter.CurseforgeDownloadCancelled)
			return true
		})
		adapter.setCurseforgeManualDownloadHandler(async () => 'verified.zip')
		assert.equal(await adapter.requestCurseforgeManualDownload(restricted), 'verified.zip')
	} finally {
		adapter.setCurseforgeManualDownloadHandler(undefined)
	}
})

test('Worlds category and Minecraft version filters reach the CurseForge search endpoint', async () => {
	const calls = []
	invoke = async (_command, args) => {
		calls.push(args)
		return args.path === 'categories'
			? { data: [{ id: 204, classId: 17, name: 'Survival' }] }
			: { data: [], pagination: { totalCount: 0 } }
	}
	await adapter.searchCurseforge({
		projectType: 'world',
		query: '',
		sort: 'relevance',
		page: 1,
		limit: 20,
		filters: [
			{ type: 'category_world_categories', option: 'Survival' },
			{ type: 'game_version', option: '1.21.1' },
		],
	})
	const query = Object.fromEntries(calls.find((call) => call.path === 'mods/search').query)
	assert.equal(query.classId, '17')
	assert.equal(query.categoryIds, '[204]')
	assert.equal(query.gameVersions, '["1.21.1"]')
})

test('exact file lookups use two batches and never download project history', async () => {
	const calls = []
	invoke = async (command, args) => {
		calls.push({ command, ...args })
		if (command.endsWith('_post')) return { data: args.path === 'mods/files' ? [file] : [mod] }
		return { data: args.path === 'mods/10' ? mod : [file], pagination: { totalCount: 500 } }
	}
	const versions = await adapter.getCurseforgeVersions(['cf-10-100', 'cf-10-100'])
	assert.deepEqual(
		versions.map((v) => v.id),
		['cf-10-100', 'cf-10-100'],
	)
	assert.equal(calls.length, 2)
	assert.ok(calls.every((c) => c.command.endsWith('_post')))
	assert.deepEqual(calls.find((c) => c.path === 'mods/files').body, { fileIds: [100] })
})
test('overview uses latest indexes rather than downloading all file pages', async () => {
	const calls = []
	invoke = async (_command, { path }) => {
		calls.push(path)
		return {
			data: path === 'mods/10' ? mod : path.endsWith('description') ? 'Description' : [file],
			pagination: { totalCount: 500 },
		}
	}
	const project = await adapter.getCurseforgeProject('cf-10')
	assert.deepEqual(project.versions, ['cf-10-100'])
	assert.deepEqual(project.game_versions, ['1.21.1'])
	assert.deepEqual(calls.sort(), ['mods/10', 'mods/10/description'])
})
test('search without category filters does not refetch categories', async () => {
	const calls = []
	invoke = async (_command, { path }) => {
		calls.push(path)
		return { data: [], pagination: { totalCount: 0 } }
	}
	await adapter.searchCurseforge({
		projectType: 'mod',
		query: '',
		sort: 'relevance',
		page: 1,
		limit: 20,
		filters: [],
	})
	assert.deepEqual(calls, ['mods/search'])
})

test('Bedrock search sends its game and class and never presents Java results', async () => {
	const calls = []
	invoke = async (_command, args) => {
		calls.push(args)
		return {
			data: [
				{ ...mod, gameId: 432 },
				{ ...mod, id: 20, gameId: 78022, classId: 4984 },
			],
			pagination: { totalCount: 2 },
		}
	}
	const result = await adapter.searchCurseforge({
		projectType: 'mod',
		edition: 'bedrock',
		query: '',
		sort: 'relevance',
		limit: 20,
		page: 1,
		filters: [],
	})
	const query = Object.fromEntries(calls[0].query)
	assert.equal(query.gameId, '78022')
	assert.equal(query.classId, '4984')
	assert.deepEqual(
		result.hits.map((hit) => hit.project_id),
		['cf-20'],
	)
})

test('Bedrock projects carry edition and map texture packs and worlds without Java loaders', async () => {
	invoke = async (_command, { path }) => ({
		data: path.endsWith('description') ? '' : { ...mod, gameId: 78022, classId: 6929 },
	})
	const project = await adapter.getCurseforgeProject('cf-10')
	assert.equal(project.minecraft_edition, 'bedrock')
	assert.equal(project.project_type, 'resourcepack')
	assert.deepEqual(project.loaders, [])
})

test('Bedrock history and game filters use Bedrock versions without Java loader fallbacks', async () => {
	invoke = async (_command, args) => ({
		data:
			args.path === 'games/78022/versions'
				? [
						{ type: 1, versions: ['1.21.100', '1.21.90'] },
						{ type: 2, versions: ['1.21.100'] },
					]
				: args.path === 'mods/10'
					? { ...mod, gameId: 78022, classId: 6929 }
					: [{ ...file, fileName: 'textures.mcpack', gameVersions: ['1.21.100'] }],
		pagination: { totalCount: 1 },
	})
	assert.deepEqual(
		(await adapter.getBedrockGameVersions()).map((v) => v.version),
		['1.21.100', '1.21.90'],
	)
	assert.deepEqual((await adapter.getCurseforgeProjectVersions('cf-10'))[0].loaders, [])
})

test('Bedrock versions sort newest first using numeric components', async () => {
	invoke = async () => ({
		data: [{ versions: ['1.21.9', '26.3', '1.20.40', '26.10', '1.21.132', '26.3'] }],
	})
	assert.deepEqual(
		(await adapter.getBedrockGameVersions()).map((v) => v.version),
		['26.10', '26.3', '1.21.132', '1.21.9', '1.20.40'],
	)
})

test('Bedrock Scripts search uses its own class and project mapping', async () => {
	let query
	invoke = async (_command, args) => {
		query = Object.fromEntries(args.query ?? [])
		return {
			data:
				args.path === 'mods/search'
					? [{ ...mod, gameId: 78022, classId: 6940 }]
					: { ...mod, gameId: 78022, classId: 6940 },
			pagination: { totalCount: 1 },
		}
	}
	const result = await adapter.searchCurseforge({
		projectType: 'datapack',
		edition: 'bedrock',
		query: '',
		sort: 'relevance',
		limit: 20,
		page: 1,
		filters: [],
	})
	assert.equal(query.classId, '6940')
	assert.deepEqual(result.hits[0].project_types, ['datapack'])
	const project = await adapter.getCurseforgeProject('cf-10')
	assert.equal(project.project_type, 'datapack')
	assert.equal(project.minecraft_edition, 'bedrock')
})

test('Bedrock resource pack resolutions get their own filter group', async () => {
	invoke = async () => ({
		data: [
			{ id: 1, name: 'x16', classId: 6929 },
			{ id: 2, name: 'x128', classId: 6929 },
			{ id: 3, name: 'Shaders', classId: 6929 },
		],
	})
	assert.deepEqual(
		(await adapter.getCurseforgeCategoryTags('resourcepack', 'bedrock')).map((c) => [
			c.name,
			c.header,
		]),
		[
			['x16', 'resolutions'],
			['x128', 'resolutions'],
			['Shaders', 'categories'],
		],
	)
})

test('Bedrock world installation forwards only the selected detected world', async () => {
	const calls = []
	invoke = async (command, args) => {
		calls.push({ command, args })
		if (command.endsWith('orbiont_download_curseforge_file')) return 'verified.mcpack'
		if (command.endsWith('import_catalog_file')) return 1
		throw new Error('Unexpected command')
	}
	const target = { root_id: 'gdk-user-123', path: 'minecraftWorlds/world-a' }
	assert.equal(await catalog.importBedrockVersion('cf-10-100', target), 1)
	assert.deepEqual(calls[1].args.target, target)
})

test('Bedrock filter layout puts categories and ascending resolutions before game versions', () => {
	assert.equal(typeof catalog.bedrockFilterLayout, 'function')
	const filters = catalog.bedrockFilterLayout([
		{ id: 'game_version', ordering: 2, options: [], toggle_groups: [{ id: 'all_versions' }] },
		{
			id: 'category_resourcepack_resolutions',
			options: [{ id: 'x128' }, { id: 'x16' }, { id: 'x64' }, { id: 'x32' }],
		},
		{ id: 'category_resourcepack_categories', options: [] },
	])
	assert.deepEqual(
		filters.map((filter) => filter.id),
		['category_resourcepack_categories', 'category_resourcepack_resolutions', 'game_version'],
	)
	assert.deepEqual(
		filters[1].options.map((option) => option.id),
		['x16', 'x32', 'x64', 'x128'],
	)
	assert.deepEqual(filters[2].toggle_groups, [])
})

test('Bedrock catalog downloads then submits the verified file to the native importer', async () => {
	const calls = []
	invoke = async (command, args) => {
		calls.push({ command, args })
		if (command.endsWith('orbiont_download_curseforge_file')) return 'C:\\Cache\\pack.mcaddon'
		if (command.endsWith('import_catalog_file')) return 1
		throw new Error(command)
	}
	assert.equal(await catalog.importBedrockVersion('cf-10-100'), 1)
	assert.deepEqual(calls, [
		{
			command: 'plugin:orbiont|orbiont_download_curseforge_file',
			args: { modId: 10, fileId: 100 },
		},
		{
			command: 'plugin:bedrock|import_catalog_file',
			args: { projectId: 10, fileId: 100, path: 'C:\\Cache\\pack.mcaddon' },
		},
	])
})

test('restricted Bedrock files accept their official page and cancellation never imports', async () => {
	const calls = []
	const restricted = {
		projectId: 10,
		fileId: 100,
		fileName: 'pack.mcaddon',
		fileSize: 3,
		sha1: 'a'.repeat(40),
		pageUrl: 'https://www.curseforge.com/minecraft-bedrock/addons/pack/files/100',
	}
	invoke = async (command) => {
		calls.push(command)
		throw { message: 'CURSEFORGE_MANUAL_DOWNLOAD:' + JSON.stringify(restricted) }
	}
	adapter.setCurseforgeManualDownloadHandler(async () => null)
	try {
		await assert.rejects(
			catalog.importBedrockVersion('cf-10-100'),
			adapter.CurseforgeDownloadCancelled,
		)
		assert.equal(calls.length, 1)
	} finally {
		adapter.setCurseforgeManualDownloadHandler(undefined)
	}
})

test('invalid Bedrock identifiers never download and native failures never report success', async () => {
	let count = 0
	invoke = async (command) => {
		count++
		if (command.endsWith('import_catalog_file')) throw { code: 'invalid_file' }
		return 'C:\\Cache\\pack.mcaddon'
	}
	for (const id of ['modrinth-id', 'cf-0-100', 'cf-10-0', 'cf-10-100-extra'])
		await assert.rejects(catalog.importBedrockVersion(id), /Invalid Bedrock/)
	assert.equal(count, 0)
	await assert.rejects(catalog.importBedrockVersion('cf-10-100'), { code: 'invalid_file' })
})
test('an explicit version list preserves historical versions and applies compatibility filters', async () => {
	const calls = []
	invoke = async (_command, args) => {
		calls.push(args)
		const index = Number(Object.fromEntries(args.query ?? []).index ?? 0)
		return {
			data: args.path === 'mods/10' ? mod : [{ ...file, id: index ? 99 : 100 }],
			pagination: { totalCount: 51 },
		}
	}
	const versions = await adapter.getCurseforgeProjectVersions('cf-10', {
		gameVersion: '1.21.1',
		loader: 'fabric',
	})
	assert.deepEqual(versions.map((v) => v.id).sort(), ['cf-10-100', 'cf-10-99'])
	assert.equal(calls.length, 3)
	for (const call of calls.filter((c) => c.path.endsWith('/files'))) {
		assert.equal(Object.fromEntries(call.query).gameVersion, '1.21.1')
		assert.equal(Object.fromEntries(call.query).modLoaderType, '4')
	}
})

test('identical concurrent lookups share requests but later actions fetch fresh data', async () => {
	let calls = 0
	invoke = async (_command, args) => {
		calls++
		await new Promise((resolve) => setTimeout(resolve, 5))
		return { data: args.path === 'mods/files' ? [file] : [mod] }
	}
	await Promise.all([
		adapter.getCurseforgeVersions(['cf-10-100']),
		adapter.getCurseforgeVersions(['cf-10-100']),
	])
	assert.equal(calls, 2)
	await adapter.getCurseforgeVersions(['cf-10-100'])
	assert.equal(calls, 4)
})

test('native error objects retain the 429 status for query retry decisions', async () => {
	invoke = async () => {
		throw { message: 'CurseForge request failed: 429 Too Many Requests', field_name: 'Theseus' }
	}
	await assert.rejects(adapter.getCurseforgeCategoryTags('mod'), /429 Too Many Requests/)
})

test('required dependencies reuse resolved files rather than fetching history again', async () => {
	const lookups = [],
		downloads = []
	const primary = { ...file, dependencies: [{ modId: 11, relationType: 3 }] }
	const dependency = { ...file, id: 101, modId: 11, fileName: 'dependency.jar' }
	invoke = async (command, args) => {
		if (command.endsWith('instance_get')) return { game_version: '1.21.1', loader: 'fabric' }
		if (command.endsWith('instance_get_projects')) return {}
		if (command.endsWith('orbiont_download_curseforge_file')) {
			downloads.push(args)
			return 'download.jar'
		}
		if (command.endsWith('instance_add_project_from_path')) return 'installed.jar'
		lookups.push(args.path)
		if (args.path === 'mods/files') return { data: [primary] }
		if (args.path === 'mods') return { data: [mod] }
		if (args.path === 'mods/11/files') return { data: [dependency] }
		if (args.path === 'mods/11') return { data: { ...mod, id: 11 } }
		throw new Error(`Unexpected request: ${args.path}`)
	}
	const result = await adapter.installCurseforgeVersionToInstance('instance', 'cf-10-100', 'mod')
	assert.deepEqual(result.dependencies, [{ projectId: 'cf-11', versionId: 'cf-11-101' }])
	assert.deepEqual(lookups.sort(), ['mods', 'mods/11', 'mods/11/files', 'mods/files'])
	assert.equal(downloads.length, 2)
})

test('a restricted file pauses for a verified manual download rather than installing HTML', async () => {
	const calls = []
	invoke = async (command, args) => {
		calls.push(command)
		if (command.endsWith('instance_get')) return { game_version: '1.21.1', loader: 'fabric' }
		if (command.endsWith('instance_get_projects')) return {}
		if (command.endsWith('orbiont_download_curseforge_file')) {
			throw {
				message:
					'CURSEFORGE_MANUAL_DOWNLOAD:' +
					JSON.stringify({
						projectId: 10,
						fileId: 100,
						fileName: 'mod.jar',
						fileSize: 3,
						sha1: 'a'.repeat(40),
						pageUrl: 'https://www.curseforge.com/minecraft/mc-mods/mod/files/100',
					}),
			}
		}
		return {
			data:
				args.path === 'mods/files'
					? [{ ...file, downloadUrl: null }]
					: args.path === 'mods'
						? [mod]
						: mod,
		}
	}
	await assert.rejects(
		adapter.installCurseforgeVersionToInstance('instance', 'cf-10-100'),
		/manual download/i,
	)
	assert.ok(calls.some((command) => command.endsWith('orbiont_download_curseforge_file')))
	assert.ok(!calls.some((command) => command.endsWith('instance_add_project_from_path')))
})

test('a pack with required manual files cannot proceed without its manual download handler', async () => {
	invoke = async () => ({
		path: 'converted.mrpack',
		skipped: [{ name: 'Required mod', projectId: 10, fileId: 100 }],
	})
	await assert.rejects(adapter.convertCurseforgeModpack('pack.zip', 'Pack'), /manual download/i)
})

test('required restricted dependencies stop installation until a verified manual file is provided', async () => {
	const added = [],
		manual = []
	const restricted = {
		projectId: 11,
		fileId: 101,
		fileName: 'dependency.jar',
		fileSize: 3,
		sha1: 'a'.repeat(40),
		pageUrl: 'https://www.curseforge.com/minecraft/mc-mods/mod/files/101',
	}
	invoke = async (command, args) => {
		if (command.endsWith('instance_get')) return { game_version: '1.21.1', loader: 'fabric' }
		if (command.endsWith('instance_get_projects')) return {}
		if (command.endsWith('instance_add_project_from_path')) {
			added.push(args)
			return 'installed.jar'
		}
		if (command.endsWith('orbiont_download_curseforge_file')) {
			if (args.modId === 11)
				throw { message: 'CURSEFORGE_MANUAL_DOWNLOAD:' + JSON.stringify(restricted) }
			return 'primary.jar'
		}
		if (args.path === 'mods/files')
			return { data: [{ ...file, dependencies: [{ modId: 11, relationType: 3 }] }] }
		if (args.path === 'mods') return { data: [mod] }
		if (args.path === 'mods/11') return { data: { ...mod, id: 11 } }
		if (args.path === 'mods/11/files')
			return {
				data: [{ ...file, id: 101, modId: 11, fileName: 'dependency.jar', downloadUrl: null }],
			}
		throw new Error('Unexpected request')
	}
	adapter.setCurseforgeManualDownloadHandler(async (file) => {
		manual.push(file)
		return null
	})
	await assert.rejects(
		adapter.installCurseforgeVersionToInstance('instance', 'cf-10-100', 'mod'),
		/canceled/,
	)
	assert.equal(added.length, 0)
	assert.equal(manual.length, 1)
	adapter.setCurseforgeManualDownloadHandler(async () => 'verified-dependency.jar')
	const result = await adapter.installCurseforgeVersionToInstance('instance', 'cf-10-100', 'mod')
	assert.equal(result.dependencies[0].versionId, 'cf-11-101')
	assert.deepEqual(
		added.map((file) => file.projectPath),
		['primary.jar', 'verified-dependency.jar'],
	)
	adapter.setCurseforgeManualDownloadHandler(undefined)
})

test('manual download requests reject untrusted redirects and share a sequential prompt queue', async () => {
	const file = {
		projectId: 10,
		fileId: 100,
		fileName: 'mod.jar',
		fileSize: 3,
		sha1: 'a'.repeat(40),
		pageUrl: 'https://www.curseforge.com/minecraft/mc-mods/mod/files/100',
	}
	assert.equal(
		adapter.parseCurseforgeManualDownload(
			'CURSEFORGE_MANUAL_DOWNLOAD:' +
				JSON.stringify({ ...file, pageUrl: 'https://evil.test/files/100' }),
		),
		null,
	)
	let active = 0,
		calls = 0
	adapter.setCurseforgeManualDownloadHandler(async () => {
		calls++
		active++
		assert.equal(active, 1)
		await new Promise((done) => setTimeout(done, 10))
		active--
		return 'verified.jar'
	})
	const results = await Promise.all([
		adapter.requestCurseforgeManualDownload(file),
		adapter.requestCurseforgeManualDownload(file),
		adapter.requestCurseforgeManualDownload({ ...file, fileId: 101 }),
	])
	assert.equal(calls, 2)
	assert.equal(results.length, 3)
	adapter.setCurseforgeManualDownloadHandler(undefined)
})

test('pack preflight resolves every required restricted file by its flattened IDs', async () => {
	const downloads = []
	invoke = async (command, args) => {
		if (command.endsWith('orbiont_convert_curseforge_pack'))
			return {
				path: 'converted.mrpack',
				skipped: [{ name: 'Required mod', projectId: 10, fileId: 100, pageUrl: null }],
			}
		if (command.endsWith('orbiont_download_curseforge_file')) {
			downloads.push(args)
			return 'verified.jar'
		}
		throw new Error('Unexpected request')
	}
	adapter.setCurseforgeManualDownloadHandler(async () => 'verified.jar')
	assert.equal(await adapter.convertCurseforgeModpack('pack.zip', 'Pack'), 'converted.mrpack')
	assert.deepEqual(downloads, [{ modId: 10, fileId: 100 }])
	adapter.setCurseforgeManualDownloadHandler(undefined)
})

test('an already installed dependency still resolves its required transitive dependencies', async () => {
	const downloads = []
	invoke = async (command, args) => {
		if (command.endsWith('instance_get')) return { game_version: '1.21.1', loader: 'fabric' }
		if (command.endsWith('instance_get_projects')) return { 'mods/dependency.jar': {} }
		if (command.endsWith('orbiont_download_curseforge_file')) {
			downloads.push(args.modId)
			return 'verified.jar'
		}
		if (command.endsWith('instance_add_project_from_path')) return 'installed.jar'
		if (args.path === 'mods/files')
			return { data: [{ ...file, dependencies: [{ modId: 11, relationType: 3 }] }] }
		if (args.path === 'mods') return { data: [mod] }
		if (args.path === 'mods/11/files')
			return {
				data: [
					{
						...file,
						modId: 11,
						id: 101,
						fileName: 'dependency.jar',
						dependencies: [{ modId: 12, relationType: 3 }],
					},
				],
			}
		if (args.path === 'mods/12/files')
			return { data: [{ ...file, modId: 12, id: 102, fileName: 'transitive.jar' }] }
		if (args.path === 'mods/12') return { data: { ...mod, id: 12 } }
		throw new Error(`Unexpected request: ${args.path}`)
	}
	await adapter.installCurseforgeVersionToInstance('instance', 'cf-10-100', 'mod')
	assert.deepEqual(downloads, [10, 12])
})

test('large exact-file batches stay within the facade limit and preserve input order', async () => {
	const ids = Array.from({ length: 2001 }, (_, index) => 1000 + index)
	const requestedSizes = []
	invoke = async (_command, { path, body }) => {
		if (path === 'mods') return { data: [mod] }
		requestedSizes.push(body.fileIds.length)
		return { data: body.fileIds.map((id) => ({ ...file, id })) }
	}
	const requested = ids.map((id) => `cf-10-${id}`)
	const result = await adapter.getCurseforgeVersions(requested)
	assert.deepEqual(requestedSizes, [2000, 1])
	assert.deepEqual(
		result.map((version) => version.id),
		requested,
	)
})
