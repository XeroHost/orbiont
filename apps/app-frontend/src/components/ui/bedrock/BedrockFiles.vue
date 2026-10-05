<script setup lang="ts">
import { FolderOpenIcon } from '@orbiont/assets'
import {
	Admonition,
	Button,
	type FileItem,
	FilePageLayout,
	provideFileManager,
	TeleportOverflowMenu,
	useVIntl,
} from '@orbiont/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'

import {
	type BedrockRoot,
	listBedrockFiles,
	readBedrockFile,
	writeBedrockFile,
} from '@/helpers/bedrock'
import { bedrockMessages as messages } from '@/helpers/bedrock-messages'

const props = defineProps<{ roots: BedrockRoot[]; busy: boolean; running: boolean }>()
const emit = defineEmits<{ openFolder: [rootId: string, path: string] }>()
const { formatMessage } = useVIntl()
const rootId = ref('')
const path = ref('')
const roots = computed(() => props.roots.filter((root) => root.kind !== 'logs'))
watch(
	roots,
	(available) => {
		if (!available.some((root) => root.id === rootId.value))
			rootId.value = (available.find((root) => root.kind === 'user') ?? available[0])?.id ?? ''
	},
	{ immediate: true },
)
watch(
	rootId,
	() => {
		path.value = ''
	},
	{ flush: 'sync' },
)
const root = computed(() => roots.value.find((candidate) => candidate.id === rootId.value))
const editingFile = ref<{ name: string; path: string } | null>(null)
const layout = ref<InstanceType<typeof FilePageLayout>>()
defineExpose({ confirmLeave: () => layout.value?.confirmDiscardChanges() ?? Promise.resolve(true) })
const revisions = new Map<string, string>()
const editable = (name: string) =>
	/\.(txt|json|json5|jsonc|lang|mcfunction|js|ts|md|log|cfg|conf|properties|ini|yaml|yml|toml)$/i.test(
		name,
	)
const files = useQuery(
	computed(() => ({
		queryKey: ['bedrock', 'files', rootId.value, path.value],
		queryFn: () => listBedrockFiles(rootId.value, path.value),
		enabled: !!rootId.value,
		retry: false,
	})),
)
const items = computed<FileItem[]>(() =>
	(files.data.value?.entries ?? []).map((file) => ({
		name: file.name,
		path: file.path,
		type: file.is_dir ? 'directory' : 'file',
		size: file.size,
		count: file.count,
		created: file.created ?? 0,
		modified: file.modified ?? 0,
		readOnly: props.running,
	})),
)
// Navigation and editing are shared with Java; unsupported filesystem actions stay hidden.
async function unavailable(): Promise<never> {
	throw new Error('Bedrock files are read-only')
}
provideFileManager({
	browseOnly: true,
	allowEditing: true,
	canEditFile: editable,
	formatFileError: (error) =>
		error instanceof Error ? error.message : formatMessage(messages.dataError),
	items,
	loading: files.isPending,
	error: files.error,
	currentPath: path,
	navigateTo: (next) => {
		path.value = next.replace(/^\/+/, '')
	},
	editingFile,
	startEditing: (file) => {
		editingFile.value = file
	},
	stopEditing: () => {
		editingFile.value = null
	},
	createItem: unavailable,
	renameItem: unavailable,
	moveItem: unavailable,
	deleteItem: unavailable,
	readFile: async (filePath) => {
		const relative = filePath.replace(/^\/+/, '')
		const document = await readBedrockFile(rootId.value, relative)
		revisions.set(`${rootId.value}/${relative}`, document.revision)
		return document.text
	},
	readFileAsBlob: unavailable,
	writeFile: async (filePath, content) => {
		const relative = filePath.replace(/^\/+/, '')
		const key = `${rootId.value}/${relative}`
		const revision = revisions.get(key)
		if (!revision) throw new Error(formatMessage(messages.reloadBeforeSave))
		try {
			const document = await writeBedrockFile(rootId.value, relative, content, revision)
			revisions.set(key, document.revision)
			void files.refetch()
		} catch (error) {
			const code = (error as { code?: string })?.code
			throw new Error(
				formatMessage(
					code === 'conflict'
						? messages.conflictError
						: code === 'game_running'
							? messages.closeToManage
							: messages.saveError,
				),
				{ cause: error },
			)
		}
	},
	downloadFile: unavailable,
	uploadFiles: unavailable,
	isReadOnly: () => props.running,
	isBusy: computed(() => props.busy || props.running),
	readOnlyReason: computed(() => formatMessage(messages.closeToManage)),
	refresh: () => {
		void files.refetch()
	},
	basePath: computed(() => root.value?.path ?? ''),
	openInFolder: (absolute) => {
		const base = root.value?.path ?? ''
		const relative = absolute.startsWith(base)
			? absolute
					.slice(base.length)
					.replace(/^[\\/]+/, '')
					.replaceAll('\\', '/')
			: path.value
		const item = items.value.find((item) => item.path === relative)
		emit(
			'openFolder',
			rootId.value,
			item?.type === 'file' ? relative.split('/').slice(0, -1).join('/') : relative,
		)
	},
})
</script>

<template>
	<p v-if="!roots.length" class="text-secondary">{{ formatMessage(messages.noData) }}</p>
	<div v-else class="flex min-w-0 flex-col gap-4">
		<p v-if="files.isPending.value" role="status">{{ formatMessage(messages.loadingData) }}</p>
		<Admonition v-if="files.data.value?.limited" type="warning">{{
			formatMessage(messages.partialData)
		}}</Admonition>
		<FilePageLayout :key="rootId" ref="layout" show-refresh-button>
			<template #location>
				<TeleportOverflowMenu
					v-if="roots.length > 1 && !editingFile"
					type="outlined"
					:label="formatMessage(messages.dataLocation)"
					:title="root?.path"
					:options="
						roots.map((candidate) => ({
							id: candidate.id,
							label: candidate.path,
							action: () => (rootId = candidate.id),
						}))
					"
				>
					<FolderOpenIcon />
				</TeleportOverflowMenu>
				<Button
					type="outlined"
					size="lg"
					:disabled="busy"
					@click="emit('openFolder', rootId, path)"
				>
					<FolderOpenIcon />{{ formatMessage(messages.openFolder) }}
				</Button>
			</template>
		</FilePageLayout>
	</div>
</template>
