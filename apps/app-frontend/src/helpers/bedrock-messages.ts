import { defineMessages } from '@orbiont/ui'

export const bedrockMessages = defineMessages({
	closeToManage: {
		id: 'app.bedrock.close-to-manage',
		defaultMessage: 'Close Minecraft to edit files, delete content or restore a copy.',
	},
	running: { id: 'app.bedrock.running', defaultMessage: 'Running' },
	reloadBeforeSave: {
		id: 'app.bedrock.reload-before-save',
		defaultMessage: 'Reload this file before saving.',
	},
	saveError: {
		id: 'app.bedrock.save-error',
		defaultMessage:
			'Could not save. The file may have changed, contain invalid JSON or be in use. Reload it and try again.',
	},
	deleteTitle: { id: 'app.bedrock.delete-title', defaultMessage: 'Delete local content' },
	deleteHelp: {
		id: 'app.bedrock.delete-help',
		defaultMessage:
			'These items will be removed from Minecraft. A recovery copy will be kept, including affected world pack settings. Worlds that depend on a deleted pack may lose its blocks or behavior; restore it before opening those worlds.',
	},
	recoveryTitle: { id: 'app.bedrock.recovery-title', defaultMessage: 'Recovery copies' },
	recoveryHelp: {
		id: 'app.bedrock.recovery-help',
		defaultMessage:
			'Original files and deleted content are kept here. Restore a copy while Minecraft is closed. Existing content and later edits are never overwritten.',
	},
	noRecoveries: { id: 'app.bedrock.no-recoveries', defaultMessage: 'No recovery copies yet.' },
	restore: { id: 'app.bedrock.restore', defaultMessage: 'Restore' },
	mutationError: {
		id: 'app.bedrock.mutation-error',
		defaultMessage:
			'Could not change this content. Close Minecraft and refresh. Your recovery copy has been kept.',
	},
	conflictError: {
		id: 'app.bedrock.conflict-error',
		defaultMessage:
			'This content has changed or the original location is occupied. The recovery copy has been kept to avoid overwriting your changes.',
	},
	worldScope: { id: 'app.bedrock.world-scope', defaultMessage: 'World: {name}' },
	edition: { id: 'app.bedrock.edition', defaultMessage: 'Minecraft edition' },
	java: { id: 'app.bedrock.java', defaultMessage: 'Java Edition' },
	bedrock: { id: 'app.bedrock.bedrock', defaultMessage: 'Bedrock Edition' },
	title: { id: 'app.bedrock.title', defaultMessage: 'Minecraft Bedrock' },
	description: {
		id: 'app.bedrock.description',
		defaultMessage: 'Play Minecraft for Windows and import your worlds and add-ons.',
	},
	checking: { id: 'app.bedrock.checking', defaultMessage: 'Looking for Minecraft for Windows…' },
	installed: { id: 'app.bedrock.installed', defaultMessage: 'Minecraft for Windows is installed' },
	missing: {
		id: 'app.bedrock.missing',
		defaultMessage: 'Install Minecraft for Windows to get started',
	},
	repair: {
		id: 'app.bedrock.repair',
		defaultMessage:
			'Repair or update your installation using Microsoft Store or the official launcher.',
	},
	unsupported: {
		id: 'app.bedrock.unsupported',
		defaultMessage:
			'Bedrock integration requires Windows. You can keep playing Java Edition on this device.',
	},
	account: {
		id: 'app.bedrock.account',
		defaultMessage:
			'Minecraft and Microsoft Store check your Bedrock license. Sign in there with the account that owns Minecraft; your Java account in this launcher may be different.',
	},
	play: { id: 'app.bedrock.play', defaultMessage: 'Play' },
	launcher: { id: 'app.bedrock.launcher', defaultMessage: 'Open official launcher' },
	store: { id: 'app.bedrock.store', defaultMessage: 'Install or update' },
	refresh: { id: 'app.bedrock.refresh', defaultMessage: 'Check again' },
	importTitle: { id: 'app.bedrock.import-title', defaultMessage: 'Worlds and add-ons' },
	importDescription: {
		id: 'app.bedrock.import-description',
		defaultMessage:
			'Choose a .mcworld, .mcpack or .mcaddon file. Minecraft will open and handle the import.',
	},
	importButton: { id: 'app.bedrock.import-button', defaultMessage: 'Import file' },
	fileFilter: { id: 'app.bedrock.file-filter', defaultMessage: 'Bedrock worlds and add-ons' },
	submitted: {
		id: 'app.bedrock.submitted',
		defaultMessage: 'File sent to Minecraft. Check the import result in the game.',
	},
	detectionError: {
		id: 'app.bedrock.detection-error',
		defaultMessage: 'Could not check the Minecraft installation. Try again.',
	},
	launchError: {
		id: 'app.bedrock.launch-error',
		defaultMessage:
			'Could not open Minecraft. Open the official launcher or Microsoft Store to check the installation.',
	},
	importError: {
		id: 'app.bedrock.import-error',
		defaultMessage:
			'Could not send the file to Minecraft. Check that Minecraft for Windows is installed and try again.',
	},
	invalidFile: {
		id: 'app.bedrock.invalid-file',
		defaultMessage: 'Choose a valid local .mcworld, .mcpack or .mcaddon archive.',
	},
	largeFile: {
		id: 'app.bedrock.large-file',
		defaultMessage: 'This file exceeds the 8 GiB import limit.',
	},
	storeError: {
		id: 'app.bedrock.store-error',
		defaultMessage: 'Could not open Microsoft Store. Open it from the Windows Start menu.',
	},
	launcherDetected: {
		id: 'app.bedrock.launcher-detected',
		defaultMessage: 'Official launcher detected. Install Minecraft for Windows there to continue.',
	},
	content: { id: 'app.bedrock.content', defaultMessage: 'Content' },
	files: { id: 'app.bedrock.files', defaultMessage: 'Files' },
	worlds: { id: 'app.bedrock.worlds', defaultMessage: 'Worlds' },
	addons: { id: 'app.bedrock.addons', defaultMessage: 'Add-ons' },
	logs: { id: 'app.bedrock.logs', defaultMessage: 'Logs' },
	update: { id: 'app.bedrock.update', defaultMessage: 'Update Minecraft' },
	updateHelp: {
		id: 'app.bedrock.update-help',
		defaultMessage:
			'In Microsoft Store, check for updates and select Minecraft for Windows. Your installed version will refresh when you return.',
	},
	version: { id: 'app.bedrock.version', defaultMessage: 'Windows installation {version}' },
	installation: { id: 'app.bedrock.installation', defaultMessage: 'Official installation' },
	resourcePack: { id: 'app.bedrock.resource-pack', defaultMessage: 'Resource pack' },
	skinPack: { id: 'app.bedrock.skin-pack', defaultMessage: 'Skin pack' },
	behaviorPack: { id: 'app.bedrock.behavior-pack', defaultMessage: 'Behavior pack' },
	development: { id: 'app.bedrock.development', defaultMessage: 'Development' },
	name: { id: 'app.bedrock.name', defaultMessage: 'Name' },
	versionColumn: { id: 'app.bedrock.version-column', defaultMessage: 'Version' },
	location: { id: 'app.bedrock.location', defaultMessage: 'Location' },
	openFolder: { id: 'app.bedrock.open-folder', defaultMessage: 'Open folder' },
	refreshData: { id: 'app.bedrock.refresh-data', defaultMessage: 'Refresh' },
	loadingData: { id: 'app.bedrock.loading-data', defaultMessage: 'Reading Minecraft data…' },
	dataError: {
		id: 'app.bedrock.data-error',
		defaultMessage: 'Could not read Minecraft data. Refresh to try again.',
	},
	folderError: {
		id: 'app.bedrock.folder-error',
		defaultMessage: 'Could not open this Minecraft folder. Refresh your data and try again.',
	},
	partialData: {
		id: 'app.bedrock.partial-data',
		defaultMessage:
			'Some folders could not be read or reached the display limit. You can inspect them in File Explorer.',
	},
	noData: {
		id: 'app.bedrock.no-data',
		defaultMessage: 'Open Minecraft once to create its data folders, then refresh here.',
	},
	noContent: { id: 'app.bedrock.no-content', defaultMessage: 'No local content found.' },
	noWorlds: {
		id: 'app.bedrock.no-worlds',
		defaultMessage: 'No local worlds found. Create one in Minecraft or import a .mcworld file.',
	},
	noAddons: {
		id: 'app.bedrock.no-addons',
		defaultMessage: 'No local add-ons found. Import a .mcpack or .mcaddon file to get started.',
	},
	noLogs: {
		id: 'app.bedrock.no-logs',
		defaultMessage:
			'No content logs found. Enable content logging in Minecraft Settings > Creator, then open your world.',
	},
	contentHelp: {
		id: 'app.bedrock.content-help',
		defaultMessage:
			'Local resource and skin packs. Activate them in Minecraft; some content belongs to individual worlds.',
	},
	addonHelp: {
		id: 'app.bedrock.addon-help',
		defaultMessage:
			'Local behavior packs and scripts. Configure them in the world settings inside Minecraft.',
	},
	search: { id: 'app.bedrock.search', defaultMessage: 'Search local content…' },
	noResults: { id: 'app.bedrock.no-results', defaultMessage: 'No matching items.' },
	dataLocation: { id: 'app.bedrock.data-location', defaultMessage: 'Minecraft data location' },
	parentFolder: { id: 'app.bedrock.parent-folder', defaultMessage: 'Parent folder' },
	emptyFolder: { id: 'app.bedrock.empty-folder', defaultMessage: 'This folder is empty.' },
	selectLog: { id: 'app.bedrock.select-log', defaultMessage: 'Select a log to read it.' },
	logTruncated: {
		id: 'app.bedrock.log-truncated',
		defaultMessage: 'Showing the last 1 MiB of this log.',
	},
	fileSize: { id: 'app.bedrock.file-size', defaultMessage: 'Size' },
	folder: { id: 'app.bedrock.folder', defaultMessage: 'Folder' },
})
