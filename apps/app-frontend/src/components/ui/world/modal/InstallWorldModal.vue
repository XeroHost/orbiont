<script setup lang="ts">
import type { Labrinth } from '@orbiont/api-client'
import { DownloadIcon, WorldIcon } from '@orbiont/assets'
import {
	Button,
	Checkbox,
	Combobox,
	commonMessages,
	defineMessages,
	injectNotificationManager,
	NewModal,
	useVIntl,
} from '@orbiont/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, onUnmounted, ref } from 'vue'

import { recentWorldsKey } from '@/components/ui/world/queries'
import { CurseforgeDownloadCancelled, downloadVersionFile } from '@/helpers/curseforge'
import { list } from '@/helpers/instance'
import type { GameInstance } from '@/helpers/types'
import {
	importWorldArchive,
	inspectWorldArchive,
	type WorldInstallAction,
	type WorldInstallPreview,
	type WorldInstallRequest,
} from '@/helpers/world-install'
import { instanceKeys } from '@/pages/instance/query-options'

const messages = defineMessages({
	title: { id: 'app.world-install.title', defaultMessage: 'Install world' },
	description: {
		id: 'app.world-install.description',
		defaultMessage:
			'Install a new world in an instance. Existing worlds are preserved unless you choose to replace one.',
	},
	instance: { id: 'app.world-install.instance', defaultMessage: 'Destination instance' },
	version: { id: 'app.world-install.version', defaultMessage: 'World version' },
	noInstances: {
		id: 'app.world-install.no-instances',
		defaultMessage: 'Create a Minecraft instance first, then return here to install the world.',
	},
	incompatible: {
		id: 'app.world-install.incompatible',
		defaultMessage:
			'This file does not list Minecraft {version} as compatible. Check the author’s version and mod requirements before continuing.',
	},
	continueAnyway: {
		id: 'app.world-install.continue-anyway',
		defaultMessage: 'Install despite the version difference',
	},
	requirements: {
		id: 'app.world-install.requirements',
		defaultMessage:
			'Required mods or modpacks must already be installed in the destination instance. Check the project description.',
	},
	conflict: {
		id: 'app.world-install.conflict',
		defaultMessage: 'A world with this name or from this project already exists.',
	},
	replaceTarget: { id: 'app.world-install.replace-target', defaultMessage: 'World to replace' },
	backup: {
		id: 'app.world-install.backup',
		defaultMessage:
			'Replace keeps a backup of the existing world. Keep existing and Cancel install nothing. Install another copy creates a numbered world.',
	},
	replace: { id: 'app.world-install.replace', defaultMessage: 'Replace' },
	keep: { id: 'app.world-install.keep', defaultMessage: 'Keep existing' },
	copy: { id: 'app.world-install.copy', defaultMessage: 'Install another copy' },
	installed: { id: 'app.world-install.installed', defaultMessage: 'World installed: {name}' },
	backupLocation: {
		id: 'app.world-install.backup-location',
		defaultMessage: 'Previous world saved in {path} inside the instance.',
	},
	openWorlds: { id: 'app.world-install.open-worlds', defaultMessage: 'View worlds' },
})
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const queryClient = useQueryClient()
const emit = defineEmits<{ navigate: [instanceId: string] }>()
const modal = ref<InstanceType<typeof NewModal>>()
const project = ref<Labrinth.Projects.v2.Project | null>(null)
const versions = ref<Labrinth.Versions.v2.Version[]>([])
const instances = ref<GameInstance[]>([])
const instanceId = ref('')
const versionId = ref('')
const preview = ref<WorldInstallPreview | null>(null)
const conflictPath = ref('')
const busy = ref(false)
const allowIncompatible = ref(false)
const installedPath = ref('')
const backupPath = ref('')
const selectedVersion = computed(() => versions.value.find((item) => item.id === versionId.value))
const selectedInstance = computed(() =>
	instances.value.find((item) => item.id === instanceId.value),
)
const incompatible = computed(
	() =>
		!!selectedVersion.value &&
		!!selectedInstance.value &&
		!selectedVersion.value.game_versions.includes(selectedInstance.value.game_version),
)
const canInstall = computed(
	() =>
		!!selectedInstance.value &&
		!!selectedVersion.value &&
		(!incompatible.value || allowIncompatible.value),
)
let archivePath = ''
let archiveVersion = ''
let resolve: ((versionId: string | null) => void) | null = null
let generation = 0

function settle(value: string | null = null) {
	const pending = resolve
	resolve = null
	generation++
	pending?.(value)
}
function cancel() {
	if (busy.value) return
	modal.value?.hide()
}
function resetSelection() {
	preview.value = null
	conflictPath.value = ''
	allowIncompatible.value = false
}
function selectInstance() {
	resetSelection()
	const preferred = versions.value.find((item) =>
		item.game_versions.includes(selectedInstance.value?.game_version ?? ''),
	)
	versionId.value = preferred?.id ?? versions.value[0]?.id ?? ''
}
async function show(request: WorldInstallRequest): Promise<string | null> {
	if (resolve || busy.value) return null
	project.value = request.project
	versions.value = request.versions
	instances.value = []
	instanceId.value = ''
	versionId.value = request.versions[0]?.id ?? ''
	installedPath.value = ''
	backupPath.value = ''
	archivePath = ''
	archiveVersion = ''
	resetSelection()
	busy.value = true
	const ticket = ++generation
	const promise = new Promise<string | null>((done) => {
		resolve = done
	})
	if (!request.autoInstall) modal.value?.show()
	try {
		instances.value = (await list()).filter(
			(item) => !item.quarantined && item.install_stage === 'installed',
		)
		if (ticket !== generation) return promise
		instanceId.value = request.instanceId ?? instances.value[0]?.id ?? ''
		selectInstance()
		allowIncompatible.value = !!request.allowIncompatible
	} catch (error) {
		handleError(error)
	} finally {
		busy.value = false
	}
	if (request.autoInstall && canInstall.value) await install()
	else modal.value?.show()
	return promise
}
async function install(action: WorldInstallAction = 'create') {
	if (busy.value || !canInstall.value || !project.value) return
	busy.value = true
	try {
		if (archiveVersion !== versionId.value) {
			archivePath = await downloadVersionFile(versionId.value)
			archiveVersion = versionId.value
		}
		if (!preview.value) {
			preview.value = await inspectWorldArchive(instanceId.value, archivePath, project.value.id)
			conflictPath.value = preview.value.conflicts[0]?.path ?? ''
			if (preview.value.conflicts.length) {
				modal.value?.show()
				return
			}
		}
		const conflict = preview.value.conflicts.find((item) => item.path === conflictPath.value)
		const result = await importWorldArchive(
			instanceId.value,
			archivePath,
			project.value.id,
			action,
			conflict,
		)
		if (result.preview) {
			preview.value = result.preview
			conflictPath.value = result.preview.conflicts[0]?.path ?? ''
			modal.value?.show()
			return
		}
		if (result.path) {
			installedPath.value = result.path
			backupPath.value = result.backup ?? ''
			modal.value?.show()
			await queryClient.invalidateQueries({ queryKey: instanceKeys.worlds(instanceId.value) })
			await queryClient.invalidateQueries({ queryKey: recentWorldsKey })
			settle(versionId.value)
		}
	} catch (error) {
		if (error instanceof CurseforgeDownloadCancelled) {
			settle()
			// A manual download can be canceled before this modal has ever opened.
			busy.value = false
			modal.value?.hide()
			return
		}
		modal.value?.show()
		handleError(error)
	} finally {
		busy.value = false
	}
}
function openWorlds() {
	emit('navigate', instanceId.value)
	modal.value?.hide()
}
onUnmounted(() => settle())
defineExpose({ show })
</script>

<template>
	<NewModal
		ref="modal"
		:header="formatMessage(messages.title)"
		max-width="560px"
		:disable-close="busy"
		@hide="settle()"
	>
		<div class="flex flex-col gap-4">
			<h3 class="m-0 flex items-center gap-2 text-contrast"><WorldIcon />{{ project?.title }}</h3>
			<template v-if="installedPath">
				<p role="status" class="m-0 text-contrast">
					{{ formatMessage(messages.installed, { name: installedPath }) }}
				</p>
				<p v-if="backupPath" class="m-0 break-words text-secondary">
					{{ formatMessage(messages.backupLocation, { path: backupPath }) }}
				</p>
			</template>
			<template v-else>
				<p class="m-0 text-secondary">{{ formatMessage(messages.description) }}</p>
				<p v-if="!instances.length && !busy" class="m-0 text-secondary">
					{{ formatMessage(messages.noInstances) }}
				</p>
				<p class="m-0 font-semibold text-contrast">
					{{ selectedInstance?.name }} · {{ selectedInstance?.game_version }}
				</p>
				<label class="flex flex-col gap-2 font-semibold text-contrast">
					{{ formatMessage(messages.version) }}
					<Combobox
						v-model="versionId"
						:options="
							versions.map((item) => ({
								value: item.id,
								label: `${item.name} · ${item.game_versions.join(', ')}`,
							}))
						"
						searchable
						sync-with-selection
						:disabled="busy"
						@update:model-value="resetSelection"
					/>
				</label>
				<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.requirements) }}</p>
				<div v-if="incompatible" class="flex flex-col gap-2 rounded-xl bg-orange-highlight p-3">
					<p class="m-0">
						{{ formatMessage(messages.incompatible, { version: selectedInstance?.game_version }) }}
					</p>
					<Checkbox
						v-model="allowIncompatible"
						:label="formatMessage(messages.continueAnyway)"
						:disabled="busy"
					/>
				</div>
				<div
					v-if="preview?.conflicts.length"
					class="flex flex-col gap-3 rounded-xl bg-surface-3 p-4"
				>
					<strong class="text-contrast">{{ formatMessage(messages.conflict) }}</strong>
					<label class="flex flex-col gap-2">
						{{ formatMessage(messages.replaceTarget) }}
						<Combobox
							v-model="conflictPath"
							:options="
								preview.conflicts.map((item) => ({
									value: item.path,
									label: `${item.name} (${item.path})`,
								}))
							"
							sync-with-selection
							:disabled="busy"
						/>
					</label>
					<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.backup) }}</p>
				</div>
			</template>
		</div>
		<template #actions>
			<div class="flex flex-wrap justify-end gap-2">
				<Button type="outlined" :disabled="busy" @click="cancel">{{
					formatMessage(installedPath ? commonMessages.closeButton : commonMessages.cancelButton)
				}}</Button>
				<Button v-if="installedPath" type="colored" color="green" @click="openWorlds">{{
					formatMessage(messages.openWorlds)
				}}</Button>
				<template v-else-if="preview?.conflicts.length">
					<Button type="outlined" :disabled="busy" @click="cancel">{{
						formatMessage(messages.keep)
					}}</Button>
					<Button
						type="colored"
						color="orange"
						:disabled="busy || !canInstall"
						@click="install('replace')"
						>{{ formatMessage(messages.replace) }}</Button
					>
					<Button
						type="colored"
						color="green"
						:disabled="busy || !canInstall"
						@click="install('copy')"
						>{{ formatMessage(messages.copy) }}</Button
					>
				</template>
				<Button
					v-else
					type="colored"
					color="green"
					:disabled="busy || !canInstall"
					@click="install()"
					><DownloadIcon />{{
						formatMessage(busy ? commonMessages.installingLabel : messages.title)
					}}</Button
				>
			</div>
		</template>
	</NewModal>
</template>
