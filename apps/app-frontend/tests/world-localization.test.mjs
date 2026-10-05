import assert from 'node:assert/strict'
import { readdirSync, readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { after, before, test } from 'node:test'

import { createServer } from 'vite'

const uiRoot = resolve(import.meta.dirname, '../../../packages/ui')
const require = createRequire(join(uiRoot, 'package.json'))
const { IntlMessageFormat } = require('intl-messageformat')
let server
let formatCategory

before(async () => {
	server = await createServer({
		configFile: false,
		root: uiRoot,
		cacheDir: join(tmpdir(), 'orbiont-world-localization-tests-vite'),
		optimizeDeps: { noDiscovery: true, include: [] },
		server: { middlewareMode: true, hmr: false },
		appType: 'custom',
		plugins: [
			{
				name: 'message-descriptor-test-boundary',
				resolveId: (id) => (id === '../composables/i18n' ? '\0message-descriptors' : undefined),
				load: (id) =>
					id === '\0message-descriptors'
						? 'export const defineMessages = (messages) => messages'
						: undefined,
			},
		],
	})
	;({ formatCategory } = await server.ssrLoadModule('/src/utils/tag-messages.ts'))
})

after(async () => {
	await server?.close()
})

const categories = [
	'Adventure',
	'Creation',
	'Game Map',
	'Modded World',
	'Parkour',
	'Puzzle',
	'Survival',
]
const spanish = [
	'Aventura',
	'Creación',
	'Mapa de juego',
	'Mundo con mods',
	'Parkour',
	'Rompecabezas',
	'Supervivencia',
]
const catalog = (directory, locale) =>
	JSON.parse(readFileSync(join(directory, locale, 'index.json'), 'utf8'))

test('CurseForge category names use translations for both provider casing and lowercase tags', () => {
	const messages = catalog(join(uiRoot, 'src/locales'), 'es-419')
	const formatter = ({ id }) => {
		assert.ok(messages[id], `Missing translated category ${id}`)
		return messages[id].defaultMessage
	}
	for (let index = 0; index < categories.length; index++) {
		assert.equal(formatCategory(formatter, categories[index]), spanish[index])
		assert.equal(formatCategory(formatter, categories[index].toLowerCase()), spanish[index])
	}
	assert.equal(formatCategory(formatter, 'Unknown provider category'), 'Unknown provider category')
})

test('all locale catalogs render world categories and plurals without falling back to English', () => {
	const directory = join(uiRoot, 'src/locales')
	for (const { name: locale } of readdirSync(directory, { withFileTypes: true }).filter((entry) =>
		entry.isDirectory(),
	)) {
		const messages = catalog(directory, locale)
		for (const category of categories) {
			formatCategory(({ id }) => {
				assert.ok(messages[id]?.defaultMessage, `${locale}: missing ${id}`)
				return new IntlMessageFormat(messages[id].defaultMessage, locale).format()
			}, category)
		}
		for (const key of ['project-type.world.capital', 'project-type.world.lowercase']) {
			assert.ok(messages[key]?.defaultMessage, `${locale}: missing ${key}`)
			for (const count of [0, 1, 2, 5, 11]) {
				const result = new IntlMessageFormat(messages[key].defaultMessage, locale).format({ count })
				assert.ok(
					typeof result === 'string' && result.length > 0,
					`${locale}: invalid plural ${count}`,
				)
			}
		}
	}
})

test('world installer messages preserve dynamic names, versions and backup paths in every locale', () => {
	const directory = resolve(import.meta.dirname, '../src/locales')
	const source = catalog(directory, 'en-US')
	const keys = Object.keys(source).filter((key) => key.startsWith('app.world-install.'))
	const values = { name: 'Test world', version: '1.21.11', path: 'saves/.backups/Test world' }
	for (const { name: locale } of readdirSync(directory, { withFileTypes: true }).filter((entry) =>
		entry.isDirectory(),
	)) {
		const messages = catalog(directory, locale)
		for (const key of keys) {
			assert.ok(messages[key]?.message, `${locale}: missing ${key}`)
			const result = new IntlMessageFormat(messages[key].message, locale).format(values)
			for (const [variable, value] of Object.entries(values)) {
				if (source[key].message.includes(`{${variable}}`))
					assert.ok(result.includes(value), `${locale}: lost ${variable} in ${key}`)
			}
		}
	}
})
