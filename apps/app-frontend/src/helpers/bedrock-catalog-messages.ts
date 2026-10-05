import { defineMessages } from '@orbiont/ui'

export const bedrockCatalogMessages = defineMessages({
	discoverAddons: { id: 'app.bedrock.catalog.discover-addons', defaultMessage: 'Discover add-ons' },
	addons: { id: 'app.bedrock.catalog.addons', defaultMessage: 'Addons' },
	scripts: { id: 'app.bedrock.catalog.scripts', defaultMessage: 'Scripts' },
	discoverScripts: {
		id: 'app.bedrock.catalog.discover-scripts',
		defaultMessage: 'Discover scripts',
	},
	world: { id: 'app.bedrock.catalog.world', defaultMessage: 'Destination world' },
	noWorlds: {
		id: 'app.bedrock.catalog.no-worlds',
		defaultMessage: 'Create a world in Minecraft first, then refresh this list.',
	},
	worldInstructions: {
		id: 'app.bedrock.catalog.world-instructions',
		defaultMessage:
			'Close Minecraft before installing. These packs will be added and activated only in the selected world. Check required dependencies and experimental settings in the project description.',
	},
	worldSuccess: {
		id: 'app.bedrock.catalog.world-success',
		defaultMessage: 'Packs added to {world}. Open Minecraft to play this world.',
	},
	gameRunning: {
		id: 'app.bedrock.catalog.game-running',
		defaultMessage: 'Close Minecraft before adding packs to a world, then try again.',
	},
	missingDependency: {
		id: 'app.bedrock.catalog.missing-dependency',
		defaultMessage:
			'This pack requires another pack that is not active in this world. Install the dependencies listed by the author first.',
	},
	discover: { id: 'app.bedrock.catalog.discover', defaultMessage: 'Discover Bedrock content' },
	description: {
		id: 'app.bedrock.catalog.description',
		defaultMessage: 'Browse add-ons, resource packs, worlds and scripts from CurseForge.',
	},
	title: {
		id: 'app.bedrock.catalog.install-title',
		defaultMessage: 'Install in Minecraft Bedrock',
	},
	version: { id: 'app.bedrock.catalog.version', defaultMessage: 'Content version' },
	instructions: {
		id: 'app.bedrock.catalog.instructions',
		defaultMessage:
			'Minecraft will open to import this content. Check the result in the game, then activate the packs in your world settings. Required packs and experimental settings are listed in the project description.',
	},
	noVersions: {
		id: 'app.bedrock.catalog.no-versions',
		defaultMessage:
			'No supported .mcaddon, .mcpack, .mcworld or ZIP files are available for this project.',
	},
	invalidArchive: {
		id: 'app.bedrock.catalog.invalid-archive',
		defaultMessage: 'This archive does not contain supported Bedrock content.',
	},
	loadError: {
		id: 'app.bedrock.catalog.load-error',
		defaultMessage: 'Could not load the project versions or worlds. Try again.',
	},
})
