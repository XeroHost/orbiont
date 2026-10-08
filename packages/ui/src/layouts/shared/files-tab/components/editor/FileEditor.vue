<template>
	<div
		ref="editorContainer"
		class="relative flex flex-col overflow-hidden rounded-[20px] border border-solid border-surface-4 shadow-sm"
	>
		<EditorFindReplace
			ref="findReplaceRef"
			v-model:is-find-open="isFindOpen"
			v-model:find-query="inFileFindQuery"
			:is-editing-image="isEditingImage"
			:readonly="isEditorReadOnly"
			:find-match-count="findMatchCount"
			:current-find-match="currentFindMatch"
			@find-next="findNext"
			@find-previous="findPrevious"
			@close="closeFind"
			@replace="replaceOne"
			@replace-all="replaceAllOccurrences"
		/>
		<component
			:is="editorComponent"
			v-if="!isEditingImage && !isLoading && !isEditorLoading && editorComponent"
			v-model:value="fileContent"
			:lang="editorLanguage"
			theme="modrinth"
			:readonly="isEditorReadOnly"
			:print-margin="false"
			:style="{ height: editorHeight, fontSize: '0.875rem' }"
			class="ace-modrinth rounded-[20px]"
			@init="onEditorInit"
		/>
		<FileImageViewer v-else-if="isEditingImage && imagePreview" :image-blob="imagePreview" />
		<div
			v-else-if="editorLoadFailed"
			class="flex flex-col items-center justify-center gap-3 rounded-[20px] bg-surface-2 p-6"
			:style="{ height: editorHeight }"
		>
			<p class="m-0 font-semibold text-contrast">{{ formatMessage(messages.loadFailedTitle) }}</p>
			<p class="m-0 text-secondary">{{ formatMessage(messages.loadFailedText) }}</p>
			<Button :disabled="isEditorLoading" @click="retryEditor">
				{{ formatMessage(commonMessages.retryButton) }}
			</Button>
		</div>
		<div
			v-else-if="isLoading || isEditorLoading || !editorComponent"
			class="flex items-center justify-center rounded-[20px] bg-surface-2"
			:style="{ height: editorHeight }"
		>
			<SpinnerIcon class="h-8 w-8 animate-spin text-secondary" />
		</div>
	</div>
</template>

<script setup lang="ts">
import { SpinnerIcon } from '@orbiont/assets'
import type { Ace } from 'ace-builds'
import {
	type Component,
	computed,
	nextTick,
	onMounted,
	onUnmounted,
	ref,
	shallowRef,
	watch,
} from 'vue'

import { Button } from '#ui/components/base/buttons'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { injectApiClient } from '#ui/providers'
import { injectNotificationManager } from '#ui/providers/web-notifications'
import { loadAceEditor } from '#ui/utils/ace-loader'
import { commonMessages } from '#ui/utils/common-messages'
import { getEditorLanguage, getFileExtension, isImageFile } from '#ui/utils/file-extensions'

import { injectFileManager } from '../../providers/file-manager'
import type { EditingFile } from '../../types'
import EditorFindReplace from './EditorFindReplace.vue'
import FileImageViewer from './FileImageViewer.vue'

const props = defineProps<{
	file: EditingFile | null
}>()

const emit = defineEmits<{
	close: []
}>()

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const ctx = injectFileManager()
const client = injectApiClient()

const messages = defineMessages({
	loadFailedTitle: {
		id: 'files.editor.load-failed-title',
		defaultMessage: 'Unable to load editor',
	},
	loadFailedText: {
		id: 'files.editor.load-failed-text',
		defaultMessage: 'Try loading the editor again.',
	},
	failedToOpenTitle: {
		id: 'files.editor.failed-to-open-title',
		defaultMessage: 'Failed to open file',
	},
	failedToOpenText: {
		id: 'files.editor.failed-to-open-text',
		defaultMessage: 'Could not load file contents.',
	},
	fileSavedTitle: {
		id: 'files.editor.file-saved-title',
		defaultMessage: 'File saved',
	},
	fileSavedText: {
		id: 'files.editor.file-saved-text',
		defaultMessage: 'Your file has been saved.',
	},
	saveFailedTitle: {
		id: 'files.editor.save-failed-title',
		defaultMessage: 'Save failed',
	},
	saveFailedText: {
		id: 'files.editor.save-failed-text',
		defaultMessage: 'Could not save the file.',
	},
	logUrlCopiedTitle: {
		id: 'files.editor.log-url-copied-title',
		defaultMessage: 'Log URL copied',
	},
	logUrlCopiedText: {
		id: 'files.editor.log-url-copied-text',
		defaultMessage: 'Your log file URL has been copied to your clipboard.',
	},
	failedToShareTitle: {
		id: 'files.editor.failed-to-share-title',
		defaultMessage: 'Failed to share file',
	},
	failedToShareText: {
		id: 'files.editor.failed-to-share-text',
		defaultMessage: 'Could not upload to mclo.gs.',
	},
})

const fileContent = ref('')
const isSaving = ref(false)
const originalContent = ref('')
const isEditingImage = ref(false)
const imagePreview = ref<Blob | null>(null)
const isLoading = ref(false)
const editorComponent = shallowRef<Component | null>(null)
const isEditorLoading = ref(false)
const editorLoadFailed = ref(false)
let loadSequence = 0
const editorInstance = ref<Ace.Editor | null>(null)
const editorContainer = ref<HTMLElement | null>(null)
const editorHeight = ref('300px')

const isFindOpen = ref(false)
const inFileFindQuery = ref('')
const findMatchCount = ref(0)
const currentFindMatch = ref(0)
const findReplaceRef = ref<{ focusFindInput: () => void; openReplace: () => void } | null>(null)

watch(inFileFindQuery, handleFindInput)

function updateEditorHeight() {
	if (editorContainer.value) {
		const top = editorContainer.value.getBoundingClientRect().top
		const padding = 24
		editorHeight.value = `${Math.max(300, window.innerHeight - top - padding)}px`
	}
}

onMounted(() => {
	nextTick(updateEditorHeight)
	window.addEventListener('resize', updateEditorHeight)
})

const editorLanguage = computed(() => {
	const ext = getFileExtension(props.file?.name ?? '')
	return ext === 'mcfunction' ? 'mcfunction' : getEditorLanguage(ext)
})
const isEditorReadOnly = computed(
	() => (ctx.isBusy?.value ?? false) || (ctx.isReadOnly?.(props.file?.path ?? '') ?? false),
)

watch(isEditorReadOnly, (readOnly) => {
	editorInstance.value?.setReadOnly(readOnly)
})

watch(
	() => props.file,
	async (newFile) => {
		if (newFile) {
			closeFind()
			await loadFileContent(newFile)
			nextTick(updateEditorHeight)
		} else {
			resetState()
		}
	},
	{ immediate: true },
)

async function loadFileContent(file: { name: string; path: string }) {
	const sequence = ++loadSequence
	isLoading.value = true
	isEditorLoading.value = false
	editorComponent.value = null
	editorLoadFailed.value = false
	try {
		window.scrollTo(0, 0)
		const extension = getFileExtension(file.name)
		const normalizedPath = file.path.startsWith('/') ? file.path : `/${file.path}`

		if (isImageFile(extension)) {
			const content = await ctx.readFileAsBlob(normalizedPath)
			if (sequence !== loadSequence) return
			isEditingImage.value = true
			imagePreview.value = content
		} else {
			isEditingImage.value = false
			const content = await ctx.readFile(normalizedPath)
			if (sequence !== loadSequence) return
			fileContent.value = content
			originalContent.value = content
			await ensureEditor(sequence)
		}
	} catch (error) {
		if (sequence !== loadSequence) return
		console.error('Error fetching file content:', error)
		addNotification({
			title: formatMessage(messages.failedToOpenTitle),
			text: ctx.formatFileError?.(error) ?? formatMessage(messages.failedToOpenText),
			type: 'error',
		})
		emit('close')
	} finally {
		if (sequence === loadSequence) isLoading.value = false
	}
}

async function ensureEditor(sequence: number) {
	isEditorLoading.value = true
	editorLoadFailed.value = false
	try {
		const editor = await loadAceEditor(editorLanguage.value)
		if (sequence === loadSequence) editorComponent.value = editor
	} catch {
		if (sequence === loadSequence) editorLoadFailed.value = true
	} finally {
		if (sequence === loadSequence) isEditorLoading.value = false
	}
}

async function retryEditor() {
	if (isEditorLoading.value || !props.file || isEditingImage.value) return
	await ensureEditor(loadSequence)
}

const hasUnsavedChanges = computed(
	() => !isEditingImage.value && !isLoading.value && fileContent.value !== originalContent.value,
)

function revertChanges() {
	fileContent.value = originalContent.value
}

function resetState() {
	loadSequence++
	editorComponent.value = null
	isEditorLoading.value = false
	editorLoadFailed.value = false
	fileContent.value = ''
	originalContent.value = ''
	isEditingImage.value = false
	imagePreview.value = null
}

function onEditorInit(editor: Ace.Editor) {
	editorInstance.value = editor
	editor.setReadOnly(isEditorReadOnly.value)

	editor.commands.addCommand({
		name: 'save',
		bindKey: { win: 'Ctrl-S', mac: 'Command-S' },
		exec: () => saveFileContent(false),
	})

	editor.commands.addCommand({
		name: 'find',
		bindKey: { win: 'Ctrl-F', mac: 'Command-F' },
		exec: () => toggleFind(),
	})

	editor.commands.addCommand({
		name: 'replace',
		bindKey: { win: 'Ctrl-H', mac: 'Command-Option-F' },
		exec: () => {
			if (isEditorReadOnly.value) return
			isFindOpen.value = true
			nextTick(() => findReplaceRef.value?.openReplace())
		},
	})
}

async function saveFileContent(exit: boolean = false) {
	if (!props.file) return
	if (isEditorReadOnly.value || isSaving.value) return
	isSaving.value = true
	const contentToSave = fileContent.value
	const fileToSave = props.file

	try {
		const normalizedPath = fileToSave.path.startsWith('/') ? fileToSave.path : `/${fileToSave.path}`
		await ctx.writeFile(normalizedPath, contentToSave)

		if (props.file !== fileToSave) return
		originalContent.value = contentToSave

		if (exit && !hasUnsavedChanges.value) {
			emit('close')
		}

		addNotification({
			title: formatMessage(messages.fileSavedTitle),
			text: formatMessage(messages.fileSavedText),
			type: 'success',
		})
	} catch (error) {
		console.error('Error saving file content:', error)
		addNotification({
			title: formatMessage(messages.saveFailedTitle),
			text: ctx.formatFileError?.(error) ?? formatMessage(messages.saveFailedText),
			type: 'error',
		})
	} finally {
		isSaving.value = false
	}
}

async function shareToMclogs() {
	if (ctx.shareToMclogs) {
		await ctx.shareToMclogs(fileContent.value)
		return
	}

	try {
		const data = await client.mclogs.logs_v1.create(fileContent.value)

		if (data.success && data.url) {
			await navigator.clipboard.writeText(data.url)
			addNotification({
				title: formatMessage(messages.logUrlCopiedTitle),
				text: formatMessage(messages.logUrlCopiedText),
				type: 'success',
			})
		} else {
			throw new Error('mclo.gs upload failed')
		}
	} catch (error) {
		console.error('Error sharing file:', error)
		addNotification({
			title: formatMessage(messages.failedToShareTitle),
			text: formatMessage(messages.failedToShareText),
			type: 'error',
		})
	}
}

function countOccurrences(content: string, query: string): number {
	if (!query) return 0
	const escaped = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
	return (content.match(new RegExp(escaped, 'gi')) ?? []).length
}

function toggleFind() {
	if (isFindOpen.value) {
		closeFind()
	} else {
		isFindOpen.value = true
		nextTick(() => findReplaceRef.value?.focusFindInput())
	}
}

function closeFind() {
	isFindOpen.value = false
	inFileFindQuery.value = ''
	findMatchCount.value = 0
	currentFindMatch.value = 0
	editorInstance.value?.find('', { wrap: true })
	editorInstance.value?.focus()
}

function replaceOne(query: string) {
	const editor = editorInstance.value
	if (!editor || isEditorReadOnly.value || findMatchCount.value === 0) return
	editor.replace(query)
	nextTick(() => {
		const count = countOccurrences(fileContent.value, inFileFindQuery.value)
		findMatchCount.value = count
		currentFindMatch.value = count > 0 ? Math.min(currentFindMatch.value, count) : 0
	})
}

function replaceAllOccurrences(query: string) {
	const editor = editorInstance.value
	if (!editor || isEditorReadOnly.value || findMatchCount.value === 0) return
	editor.replaceAll(query)
	nextTick(() => {
		const count = countOccurrences(fileContent.value, inFileFindQuery.value)
		findMatchCount.value = count
		currentFindMatch.value = count > 0 ? 1 : 0
		if (count > 0) {
			editor.find(inFileFindQuery.value, { wrap: true, caseSensitive: false })
		}
	})
}

function handleFindInput() {
	const editor = editorInstance.value
	if (!editor) return

	const query = inFileFindQuery.value
	if (!query) {
		findMatchCount.value = 0
		currentFindMatch.value = 0
		editor.find('', { wrap: true })
		return
	}

	const count = countOccurrences(fileContent.value, query)
	findMatchCount.value = count

	if (count > 0) {
		editor.find(query, { wrap: true, caseSensitive: false })
		currentFindMatch.value = 1
	} else {
		currentFindMatch.value = 0
	}
}

function findNext() {
	const editor = editorInstance.value
	if (!editor || findMatchCount.value === 0) return
	editor.findNext()
	currentFindMatch.value = (currentFindMatch.value % findMatchCount.value) + 1
}

function findPrevious() {
	const editor = editorInstance.value
	if (!editor || findMatchCount.value === 0) return
	editor.findPrevious()
	currentFindMatch.value =
		((currentFindMatch.value - 2 + findMatchCount.value) % findMatchCount.value) + 1
}

function close() {
	resetState()
	emit('close')
}

onUnmounted(() => {
	window.removeEventListener('resize', updateEditorHeight)
	editorInstance.value = null
	resetState()
})

defineExpose({
	isSaving,
	saveFileContent,
	shareToMclogs,
	close,
	isEditingImage,
	isFindOpen,
	fileContent,
	hasUnsavedChanges,
	revertChanges,
	toggleFind,
})
</script>
