import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

test('every world installer IPC command is registered and included in the default capability', () => {
	const helper = readFileSync(new URL('../src/helpers/world-install.ts', import.meta.url), 'utf8')
	const build = readFileSync(new URL('../../app/build.rs', import.meta.url), 'utf8')
	const handler = readFileSync(new URL('../../app/src/api/worlds.rs', import.meta.url), 'utf8')
	const capability = JSON.parse(
		readFileSync(new URL('../../app/capabilities/plugins.json', import.meta.url), 'utf8'),
	)
	const start = build.indexOf('"worlds",')
	const plugin = build.slice(start, build.indexOf('.plugin(', start))
	const commands = [...helper.matchAll(/plugin:worlds\|(\w+)/g)].map((match) => match[1])
	assert.ok(commands.length >= 2)
	for (const command of commands) {
		assert.ok(plugin.includes(`"${command}"`), `${command} missing from generated permissions`)
		assert.ok(handler.includes(`${command},`), `${command} missing from invoke handler`)
	}
	assert.ok(plugin.includes('DefaultPermissionRule::AllowAllCommands'))
	assert.ok(capability.permissions.includes('worlds:default'))
})
