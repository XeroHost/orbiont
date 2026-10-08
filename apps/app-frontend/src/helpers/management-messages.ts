import { defineMessages } from '@orbiont/ui'

export const managementMessages = defineMessages({
	title: { id: 'local-management.title', defaultMessage: 'Recovery and storage' },
	recovery: { id: 'local-management.recovery', defaultMessage: 'Recoverable copies' },
	storage: { id: 'local-management.storage', defaultMessage: 'Storage' },
	retentionDays: { id: 'local-management.retention.days', defaultMessage: 'Older than (days)' },
	retentionKeep: {
		id: 'local-management.retention.keep',
		defaultMessage: 'Keep newest copies per file or world',
	},
	retentionHelp: {
		id: 'local-management.retention.help',
		defaultMessage:
			'Select old recoverable copies for review. Nothing is removed automatically; incomplete copies and installed content are preserved.',
	},
	retentionSelect: { id: 'local-management.retention.select', defaultMessage: 'Select old copies' },
	loading: { id: 'local-management.loading', defaultMessage: 'Loading local data…' },
	error: {
		id: 'local-management.error',
		defaultMessage: 'The operation failed. Your selection has been kept. Refresh and try again.',
	},
	empty: { id: 'local-management.empty', defaultMessage: 'No recoverable copies were found.' },
	partial: {
		id: 'local-management.partial',
		defaultMessage: 'This list or size estimate is incomplete.',
	},
	preview: { id: 'local-management.preview', defaultMessage: 'Preview' },
	restore: { id: 'local-management.restore', defaultMessage: 'Confirm restoration' },
	restoreHelp: {
		id: 'local-management.restore-help',
		defaultMessage:
			'Review the destination and copy below before restoring. Later changes or an occupied destination may block restoration.',
	},
	removeHelp: {
		id: 'local-management.remove-help',
		defaultMessage:
			'Only selected recoverable copies and retained imports can be removed. Removing them permanently prevents their recovery.',
	},
	remove: { id: 'local-management.remove', defaultMessage: 'Remove selected items' },
	confirmRemoval: {
		id: 'local-management.confirm-removal',
		defaultMessage: 'Confirm permanent removal',
	},
	selected: {
		id: 'local-management.selected',
		defaultMessage: '{count, plural, one {# selected item} other {# selected items}}',
	},
	size: { id: 'local-management.size', defaultMessage: '{bytes, number} bytes' },
	data: { id: 'local-management.category.data', defaultMessage: 'Game data' },
	cache: { id: 'local-management.category.cache', defaultMessage: 'Shared launcher cache' },
	backup: { id: 'local-management.category.recovery', defaultMessage: 'Recoverable copies' },
	imports: { id: 'local-management.category.import', defaultMessage: 'Retained imports' },
	versions: {
		id: 'local-management.category.retained-version',
		defaultMessage: 'Retained pack versions',
	},
	origin: {
		id: 'local-management.origin',
		defaultMessage:
			'{source, select, world_pack_install {World pack installation} java_editor {Java editor} other {Bedrock local management}}',
	},
	state: {
		id: 'local-management.state',
		defaultMessage:
			'{state, select, prepared {Prepared} applied {Applied} rollback_failed {Rollback failed} restored {Restored} partial {Incomplete} legacy {Legacy copy} other {Available}}',
	},
	loadMore: { id: 'local-management.load-more', defaultMessage: 'Load more copies' },
	reloadBeforeSave: {
		id: 'instance.files.reload-before-save',
		defaultMessage: 'Reopen the file before saving.',
	},
	editorConflict: {
		id: 'instance.files.editor-conflict',
		defaultMessage:
			'The file changed outside the editor. Your draft is kept. Copy it before reopening the file to review the latest version.',
	},
	saveError: {
		id: 'instance.files.editor-save-error',
		defaultMessage: 'Could not save the file. Your draft is kept.',
	},
	diagnostics: { id: 'bedrock.diagnostics.title', defaultMessage: 'Launch diagnostics' },
	launchState: {
		id: 'bedrock.diagnostics.state',
		defaultMessage:
			'{state, select, idle {Ready} requested {Launch requested} starting {Waiting for Minecraft} running {Minecraft is running} timeout {Minecraft did not start before the timeout} failed {Launch failed} other {Checking launch}}',
	},
	launchTime: {
		id: 'bedrock.diagnostics.time',
		defaultMessage: 'Elapsed: {seconds, number} seconds',
	},
	repairHelp: {
		id: 'bedrock.diagnostics.repair-help',
		defaultMessage:
			'Open the official launcher to check your session, or open Microsoft Store to install updates or repair Minecraft. Refresh detection when finished.',
	},
	export: { id: 'bedrock.diagnostics.export', defaultMessage: 'Export diagnostic report' },
	packDetails: { id: 'bedrock.packs.details', defaultMessage: 'Versions and dependencies' },
	dependencies: { id: 'bedrock.packs.dependencies', defaultMessage: 'Dependencies' },
	activations: { id: 'bedrock.packs.activations', defaultMessage: 'World activations' },
	packState: {
		id: 'bedrock.packs.state',
		defaultMessage: '{active, select, true {Active version} other {Retained version}}',
	},
	none: { id: 'bedrock.packs.none', defaultMessage: 'None reported' },
})
