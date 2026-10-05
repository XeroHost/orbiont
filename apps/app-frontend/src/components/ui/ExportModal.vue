<script setup>
import { CheckIcon, FolderOpenIcon, XIcon } from '@orbiont/assets'
import { productName } from '@orbiont/branding'
import {
	Button,
	commonMessages,
	defineMessages,
	FileTreeSelect,
	injectNotificationManager,
	injectPopupNotificationManager,
	Input,
	NewModal,
	Textarea,
	useVIntl,
} from '@orbiont/ui'
import { save } from '@tauri-apps/plugin-dialog'
import { computed, ref, shallowRef } from 'vue'

import { PackageIcon } from '@/assets/icons'
import { export_instance_mrpack, get_pack_export_candidates } from '@/helpers/instance'
import { getInstanceOptifine, optifineMessages } from '@/helpers/optifine'
import { getPackSaveFilters, PACK_FORMATS, resolvePackExport } from '@/helpers/pack-formats'
import { highlightInFolder } from '@/helpers/utils'

const { handleError } = injectNotificationManager()
const popupNotificationManager = injectPopupNotificationManager()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	header: { id: 'app.export-modal.header', defaultMessage: 'Export modpack' },
	formatLabel: { id: 'app.export-modal.format-label', defaultMessage: 'Export format' },
	orbpackFormat: {
		id: 'app.export-modal.format.orbpack',
		defaultMessage: '{productName}',
	},
	orbpackDescription: {
		id: 'app.export-modal.format.orbpack-description',
		defaultMessage:
			'A portable {productName} pack containing the selected files. Open it directly in the launcher.',
	},
	mrpackDescription: {
		id: 'app.export-modal.format.mrpack-description',
		defaultMessage:
			'For launchers supporting Modrinth packs. Includes download references and selected extra files.',
	},
	mrpackFormat: {
		id: 'app.export-modal.format.mrpack',
		defaultMessage: 'Modrinth',
	},
	curseforgeFormat: {
		id: 'app.export-modal.format.curseforge',
		defaultMessage: 'CurseForge',
	},
	curseforgeDescription: {
		id: 'app.export-modal.format.curseforge-description',
		defaultMessage: 'CurseForge ZIP with manifest.json and the selected files inside overrides.',
	},
	modpackNameLabel: { id: 'app.export-modal.modpack-name-label', defaultMessage: 'Modpack name' },
	modpackNamePlaceholder: {
		id: 'app.export-modal.modpack-name-placeholder',
		defaultMessage: 'Modpack name',
	},
	versionNumberLabel: {
		id: 'app.export-modal.version-number-label',
		defaultMessage: 'Version number',
	},
	versionNumberPlaceholder: {
		id: 'app.export-modal.version-number-placeholder',
		defaultMessage: '1.0.0',
	},
	descriptionPlaceholder: {
		id: 'app.export-modal.description-placeholder',
		defaultMessage: 'Enter modpack description...',
	},
	exportButton: { id: 'app.export-modal.export-button', defaultMessage: 'Export' },
	exportComplete: {
		id: 'app.export-modal.export-complete',
		defaultMessage: 'Export complete',
	},
	exportCompleteDescription: {
		id: 'app.export-modal.export-complete-description',
		defaultMessage: '{name} was exported successfully.',
	},
})

const props = defineProps({
	instance: {
		type: Object,
		required: true,
	},
})

defineExpose({
	show: () => {
		resetExportState()
		hasOptifine.value = false
		exportModal.value.show()
		void initFiles().catch(handleError)
		void getInstanceOptifine(props.instance.id)
			.then((value) => {
				hasOptifine.value = !!value
			})
			.catch(handleError)
	},
})

const exportModal = ref(null)
const nameInput = ref(props.instance.name)
const exportDescription = ref('')
const versionInput = ref('1.0.0')
const exportFormat = ref('orbpack')
const hasOptifine = ref(false)
const formatOptions = computed(() =>
	PACK_FORMATS.map((format) => ({
		...format,
		label: formatMessage(messages[`${format.value}Format`], { productName }),
	})),
)
const formatDescription = computed(() =>
	formatMessage(messages[`${exportFormat.value}Description`], { productName }),
)
const files = shallowRef([])
const includedFilePaths = ref([])
const excludedFilePaths = ref([])
const fileTreeKey = ref(0)
const filesLoadId = ref(0)
const directoryEntries = new Map()
const currentDirectory = ref('')

async function initFiles() {
	const loadId = ++filesLoadId.value
	const exportCandidates = await get_pack_export_candidates(props.instance.id)
	if (loadId !== filesLoadId.value) return

	files.value = exportCandidates
	directoryEntries.set('', exportCandidates)
	currentDirectory.value = ''
	includedFilePaths.value = files.value
		.filter((file) => !file.disabled && file.defaultSelected)
		.map((file) => file.path)
}

const exportPack = async () => {
	const selectedFormat = exportFormat.value
	const extension = PACK_FORMATS.find((format) => format.value === selectedFormat).extension
	const selectedPath = await save({
		defaultPath: `${nameInput.value} ${versionInput.value}.${extension}`,
		filters: getPackSaveFilters(selectedFormat),
	})

	if (selectedPath) {
		const { path: outputPath, format } = resolvePackExport(selectedPath, selectedFormat)
		exportModal.value.hide()

		try {
			await export_instance_mrpack(
				props.instance.id,
				outputPath,
				includedFilePaths.value,
				excludedFilePaths.value,
				versionInput.value,
				exportDescription.value,
				nameInput.value,
				format,
			)

			const fileName = outputPath.split(/[\\/]/).pop() ?? outputPath
			popupNotificationManager.addPopupNotification({
				title: formatMessage(messages.exportComplete),
				text: formatMessage(messages.exportCompleteDescription, { name: fileName }),
				type: 'success',
				buttons: [
					{
						label: formatMessage(commonMessages.openInFolderButton),
						icon: FolderOpenIcon,
						action: () => highlightInFolder(outputPath).catch(handleError),
					},
				],
			})
		} catch (error) {
			handleError(error)
		}
	}
}

function resetExportState() {
	nameInput.value = props.instance.name
	exportDescription.value = ''
	versionInput.value = '1.0.0'
	exportFormat.value = 'orbpack'
	files.value = []
	includedFilePaths.value = []
	excludedFilePaths.value = []
	fileTreeKey.value += 1
	directoryEntries.clear()
	currentDirectory.value = ''
}

async function loadExportDirectory(path) {
	const normalizedPath = normalizeExportPath(path)
	currentDirectory.value = normalizedPath

	const cachedEntries = directoryEntries.get(normalizedPath)
	if (cachedEntries) {
		files.value = cachedEntries
		return
	}

	const loadId = filesLoadId.value
	files.value = []

	try {
		const childItems = await get_pack_export_candidates(
			props.instance.id,
			normalizedPath || undefined,
		)
		if (loadId !== filesLoadId.value) return

		directoryEntries.set(normalizedPath, childItems)
		if (currentDirectory.value === normalizedPath) {
			files.value = childItems
		}
	} catch {
		if (currentDirectory.value === normalizedPath) files.value = []
	}
}

function normalizeExportPath(path) {
	return path.replaceAll('\\', '/').split('/').filter(Boolean).join('/')
}
</script>

<template>
	<NewModal
		ref="exportModal"
		:header="formatMessage(messages.header)"
		scrollable
		width="46rem"
		max-width="calc(100vw - 2rem)"
	>
		<div class="flex flex-col gap-4">
			<div class="labeled_input w-full">
				<p id="pack-export-format-label" class="text-contrast font-semibold">
					{{ formatMessage(messages.formatLabel) }}
				</p>
				<div
					class="grid grid-cols-3 gap-2"
					role="radiogroup"
					aria-labelledby="pack-export-format-label"
				>
					<label
						v-for="option in formatOptions"
						:key="option.value"
						class="relative flex min-w-0 cursor-pointer flex-col gap-2 rounded-xl border border-solid p-3 transition-colors has-[:focus-visible]:outline has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-brand"
						:class="
							exportFormat === option.value
								? 'border-brand bg-brand-highlight text-contrast'
								: 'border-surface-4 bg-surface-2 text-primary hover:bg-surface-3'
						"
					>
						<input
							v-model="exportFormat"
							type="radio"
							name="pack-export-format"
							:value="option.value"
							class="sr-only"
						/>
						<span class="flex min-w-0 items-center justify-between gap-2 font-semibold">
							<span class="break-words">{{ option.label }}</span>
							<CheckIcon v-if="exportFormat === option.value" class="size-4 shrink-0" />
						</span>
						<span class="text-sm text-secondary">.{{ option.extension }}</span>
					</label>
				</div>
				<p class="m-0 mt-2 text-secondary">{{ formatDescription }}</p>
				<p v-if="hasOptifine" class="m-0 mt-2 text-secondary">
					{{ formatMessage(optifineMessages.exportNotice) }}
				</p>
			</div>
			<div class="grid grid-cols-2 gap-4">
				<div class="labeled_input w-full">
					<p class="text-contrast font-semibold">{{ formatMessage(messages.modpackNameLabel) }}</p>
					<Input
						v-model="nameInput"
						type="text"
						:placeholder="formatMessage(messages.modpackNamePlaceholder)"
						clearable
						wrapper-class="w-full"
					/>
				</div>
				<div class="labeled_input w-full">
					<p class="text-contrast font-semibold">
						{{ formatMessage(messages.versionNumberLabel) }}
					</p>
					<Input
						v-model="versionInput"
						type="text"
						:placeholder="formatMessage(messages.versionNumberPlaceholder)"
						clearable
						wrapper-class="w-full"
					/>
				</div>
			</div>
			<div class="flex flex-col gap-2 min-w-0">
				<p class="m-0 text-contrast font-semibold">
					{{ formatMessage(commonMessages.descriptionLabel) }}
				</p>
				<Textarea
					v-model="exportDescription"
					:placeholder="formatMessage(messages.descriptionPlaceholder)"
					wrapper-class="w-full"
				/>
			</div>
			<FileTreeSelect
				:key="fileTreeKey"
				v-model="includedFilePaths"
				v-model:excluded-paths="excludedFilePaths"
				class="min-w-0"
				:items="files"
				lazy
				@navigate="loadExportDirectory"
			/>
		</div>
		<template #actions>
			<div class="flex items-center justify-end gap-2">
				<Button type="outlined" @click="exportModal.hide">
					<XIcon />
					{{ formatMessage(commonMessages.cancelButton) }}
				</Button>
				<Button type="colored" color="brand" @click="exportPack">
					<PackageIcon />
					{{ formatMessage(messages.exportButton) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>
