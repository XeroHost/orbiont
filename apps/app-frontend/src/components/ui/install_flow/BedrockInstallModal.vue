<script setup lang="ts">
import type { Labrinth } from '@orbiont/api-client'
import { DownloadIcon } from '@orbiont/assets'
import { Admonition, Button, Combobox, commonMessages, NewModal, useVIntl } from '@orbiont/ui'
import { computed, onUnmounted, ref } from 'vue'

import { type BedrockItem, getBedrockStatus, getBedrockWorkspace } from '@/helpers/bedrock'
import { type BedrockInstallRequest, importBedrockVersion } from '@/helpers/bedrock-catalog'
import { bedrockCatalogMessages as messages } from '@/helpers/bedrock-catalog-messages'
import { bedrockMessages } from '@/helpers/bedrock-messages'
import { CurseforgeDownloadCancelled, getCurseforgeProjectVersions } from '@/helpers/curseforge'

const { formatMessage } = useVIntl()
const modal = ref<InstanceType<typeof NewModal>>()
const project = ref<Labrinth.Projects.v2.Project | null>(null)
const versions = ref<Labrinth.Versions.v2.Version[]>([])
const selected = ref('')
const worlds = ref<BedrockItem[]>([])
const selectedWorld = ref('')
const installsWorld = computed(() => project.value?.project_type === 'world')
const worldOptions = computed(() =>
	worlds.value.map((world) => ({
		value: JSON.stringify([world.root_id, world.path]),
		label: world.name,
	})),
)
const targetWorld = computed(() =>
	worlds.value.find((world) => JSON.stringify([world.root_id, world.path]) === selectedWorld.value),
)
const loading = ref(false)
const busy = ref(false)
const ready = ref(false)
const submitted = ref(false)
const error = ref('')
const options = computed(() =>
	versions.value.map((version) => ({
		value: version.id,
		label: `${version.name} · ${version.game_versions.join(', ')}`,
	})),
)
let resolve: ((value: string | null) => void) | undefined
let generation = 0
function close() {
	if (busy.value) return
	generation++
	resolve?.(submitted.value ? selected.value : null)
	resolve = undefined
}
async function load(initialVersionId?: string | null) {
	if (!project.value) return
	const ticket = ++generation
	loading.value = true
	error.value = ''
	try {
		const [status, files, workspace] = await Promise.all([
			getBedrockStatus(),
			getCurseforgeProjectVersions(project.value.id),
			installsWorld.value ? Promise.resolve(null) : getBedrockWorkspace(),
		])
		if (ticket !== generation) return
		ready.value = !!status.supported && !!status.game?.can_launch
		worlds.value = workspace?.items.filter((item) => item.kind === 'world') ?? []
		if (!worldOptions.value.some((option) => option.value === selectedWorld.value))
			selectedWorld.value = ''
		versions.value = files.filter((file) =>
			file.files.some((entry) => /\.(mcaddon|mcpack|mcworld|zip)$/i.test(entry.filename)),
		)
		selected.value =
			versions.value.find((v) => v.id === initialVersionId)?.id ?? versions.value[0]?.id ?? ''
		if (!ready.value)
			error.value = formatMessage(
				status.supported ? bedrockMessages.missing : bedrockMessages.unsupported,
			)
	} catch {
		if (ticket === generation) error.value = formatMessage(messages.loadError)
	} finally {
		if (ticket === generation) loading.value = false
	}
}
async function show(request: BedrockInstallRequest): Promise<string | null> {
	if (resolve) return null
	project.value = request.project
	versions.value = []
	selected.value = ''
	worlds.value = []
	selectedWorld.value = ''
	ready.value = false
	submitted.value = false
	const promise = new Promise<string | null>((done) => {
		resolve = done
	})
	modal.value?.show()
	await load(request.versionId)
	return promise
}
async function install() {
	if (busy.value || !ready.value || !selected.value || (!installsWorld.value && !targetWorld.value))
		return
	busy.value = true
	error.value = ''
	try {
		await importBedrockVersion(
			selected.value,
			installsWorld.value
				? undefined
				: { root_id: targetWorld.value!.root_id, path: targetWorld.value!.path },
		)
		submitted.value = true
	} catch (cause) {
		if (!(cause instanceof CurseforgeDownloadCancelled)) {
			const code = (cause as { code?: string })?.code
			error.value = formatMessage(
				code === 'insufficient_space'
					? bedrockMessages.insufficientSpace
					: code === 'invalid_file'
						? messages.invalidArchive
						: code === 'game_running'
							? messages.gameRunning
							: code === 'missing_dependency'
								? messages.missingDependency
								: code === 'file_too_large'
									? bedrockMessages.largeFile
									: bedrockMessages.importError,
			)
		}
	} finally {
		busy.value = false
	}
}
onUnmounted(() => {
	generation++
	resolve?.(null)
})
defineExpose({ show })
</script>

<template>
	<NewModal
		ref="modal"
		:header="formatMessage(messages.title)"
		:closable="!busy"
		:close-on-click-outside="!busy"
		@hide="close"
	>
		<div class="flex w-[32rem] max-w-full flex-col gap-4" :aria-busy="busy || loading">
			<h3 class="m-0 text-contrast">{{ project?.title }}</h3>
			<p class="m-0 text-secondary">
				{{ formatMessage(installsWorld ? messages.instructions : messages.worldInstructions) }}
			</p>
			<p v-if="loading" role="status">{{ formatMessage(commonMessages.loadingLabel) }}</p>
			<template v-else-if="!submitted">
				<template v-if="!installsWorld">
					<div role="group" aria-labelledby="bedrock-world-label" class="flex flex-col gap-2">
						<span id="bedrock-world-label" class="font-semibold text-contrast">{{
							formatMessage(messages.world)
						}}</span>
						<Combobox
							v-model="selectedWorld"
							:options="worldOptions"
							:placeholder="formatMessage(messages.world)"
							:disabled="busy || !worldOptions.length"
						/>
					</div>
					<p v-if="!worldOptions.length" class="m-0 text-secondary">
						{{ formatMessage(messages.noWorlds) }}
					</p>
				</template>
				<div role="group" aria-labelledby="bedrock-version-label" class="flex flex-col gap-2">
					<span id="bedrock-version-label" class="font-semibold text-contrast">{{
						formatMessage(messages.version)
					}}</span>
					<Combobox
						id="bedrock-version"
						v-model="selected"
						:options="options"
						:disabled="busy || !options.length"
					/>
				</div>
				<p v-if="!options.length" class="text-secondary">
					{{ formatMessage(messages.noVersions) }}
				</p>
			</template>
			<Admonition v-if="error" type="warning" role="alert">{{ error }}</Admonition>
			<Admonition v-if="submitted" type="success" role="status">{{
				installsWorld
					? formatMessage(bedrockMessages.submitted)
					: formatMessage(messages.worldSuccess, { world: targetWorld?.name ?? '' })
			}}</Admonition>
			<div class="flex justify-end gap-2">
				<Button
					v-if="
						(error || (!installsWorld && !worldOptions.length)) && !busy && !loading && !submitted
					"
					@click="load(selected)"
					>{{ formatMessage(commonMessages.retryButton) }}</Button
				>
				<Button :disabled="busy" @click="modal?.hide()">{{
					formatMessage(submitted ? commonMessages.closeButton : commonMessages.cancelButton)
				}}</Button>
				<Button
					v-if="!submitted"
					type="colored"
					color="brand"
					:disabled="busy || loading || !ready || !selected || (!installsWorld && !targetWorld)"
					@click="install"
					><DownloadIcon />{{
						formatMessage(busy ? commonMessages.installingLabel : commonMessages.installButton)
					}}</Button
				>
			</div>
		</div>
	</NewModal>
</template>
