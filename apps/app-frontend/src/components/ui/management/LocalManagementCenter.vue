<script setup lang="ts">
import {
	Admonition,
	Button,
	Checkbox,
	commonMessages,
	NewModal,
	useFormatBytes,
	useVIntl,
} from '@orbiont/ui'
import { computed, ref, watch } from 'vue'

import type {
	LocalCopy,
	LocalManagementAdapter,
	LocalPreview,
	LocalStorageEntry,
} from '@/helpers/local-management'
import { managementMessages as messages } from '@/helpers/management-messages'
import { retentionSelection, validRetentionPolicy } from '@/helpers/recovery-retention'

const props = defineProps<{ adapter: LocalManagementAdapter; disabled?: boolean }>()
const { formatMessage, locale } = useVIntl()
const formatBytes = useFormatBytes()
const modal = ref<InstanceType<typeof NewModal>>()
const tab = ref<'recovery' | 'storage'>('recovery')
const busy = ref(false)
const failed = ref(false)
const copies = ref<LocalCopy[]>([])
const total = ref(0)
const incomplete = ref(false)
const storage = ref<LocalStorageEntry[]>([])
const categories = ref<Record<string, number>>({})
const preview = ref<LocalPreview | null>(null)
const selection = ref(new Set<string>())
const confirmRemoval = ref(false)
const retentionDays = ref(30)
const retentionKeep = ref(5)
const retentionValid = computed(() =>
	validRetentionPolicy({ days: retentionDays.value, keep: retentionKeep.value }),
)
const key = (item: LocalCopy) => `${item.rootId}/${item.id}`
const selected = computed(() =>
	storage.value.filter((entry) => entry.removable && selection.value.has(key(entry))),
)
const categoryMessage = (category: string) =>
	({
		data: messages.data,
		content: messages.data,
		cache: messages.cache,
		recovery: messages.backup,
		import: messages.imports,
		retained_version: messages.versions,
	})[category] ?? messages.data
const date = (seconds: number) =>
	new Intl.DateTimeFormat(locale.value, { dateStyle: 'short', timeStyle: 'short' }).format(
		new Date(seconds * 1000),
	)
async function run(operation: () => Promise<void>) {
	if (busy.value) return
	busy.value = true
	failed.value = false
	try {
		await operation()
	} catch {
		failed.value = true
	} finally {
		busy.value = false
	}
}
async function load(append = false) {
	if (tab.value === 'recovery') {
		const result = await props.adapter.list(append ? copies.value.length : 0)
		copies.value = append ? [...copies.value, ...result.entries] : result.entries
		total.value = result.total
		incomplete.value = result.incomplete
	} else {
		const result = await props.adapter.storage()
		storage.value = result.entries
		categories.value = result.categories
		incomplete.value = result.incomplete
	}
}
async function show(next: 'recovery' | 'storage' = 'recovery') {
	if (busy.value) return
	tab.value = next
	preview.value = null
	selection.value.clear()
	confirmRemoval.value = false
	try {
		const saved = JSON.parse(
			localStorage.getItem(`recovery-retention-${props.adapter.edition}`) ?? 'null',
		)
		if (saved && validRetentionPolicy(saved)) {
			retentionDays.value = saved.days
			retentionKeep.value = saved.keep
		}
	} catch {
		// Unavailable storage must not prevent opening the management center.
	}
	modal.value?.show()
	await run(() => load())
}
async function selectOldCopies() {
	if (props.disabled || busy.value || !retentionValid.value) return
	// A fresh review invalidates any earlier deletion approval, even if it fails.
	selection.value.clear()
	confirmRemoval.value = false
	await run(async () => {
		const adapter = props.adapter
		const all: LocalCopy[] = []
		const seen = new Set<string>()
		let expected = 0
		do {
			const result = await adapter.list(all.length)
			if (adapter !== props.adapter) throw new Error('Management destination changed')
			if (
				result.incomplete ||
				result.total > 20000 ||
				(!result.entries.length && all.length < result.total) ||
				all.length + result.entries.length > 20000
			) {
				incomplete.value = true
				throw new Error('Recovery inventory is incomplete')
			}
			for (const entry of result.entries) {
				if (seen.has(key(entry))) {
					incomplete.value = true
					throw new Error('Recovery inventory changed during pagination')
				}
				seen.add(key(entry))
			}
			expected = result.total
			all.push(...result.entries)
		} while (all.length < expected)
		await load()
		if (adapter !== props.adapter) throw new Error('Management destination changed')
		if (incomplete.value) throw new Error('Storage inventory is incomplete')
		const policy = { days: retentionDays.value, keep: retentionKeep.value }
		selection.value = retentionSelection(storage.value, all, policy)
		confirmRemoval.value = false
		try {
			localStorage.setItem(`recovery-retention-${props.adapter.edition}`, JSON.stringify(policy))
		} catch {
			// The reviewed selection remains usable without persisted preferences.
		}
	})
}
async function selectCopy(copy: LocalCopy) {
	preview.value = null
	await run(async () => {
		preview.value = await props.adapter.preview(copy)
	})
}
async function restore() {
	const candidate = preview.value
	if (props.disabled || !candidate?.canRestore) return
	await run(async () => {
		await props.adapter.restore(candidate)
		preview.value = null
		await load()
	})
}
function toggle(entry: LocalStorageEntry) {
	if (!entry.removable || busy.value || props.disabled) return
	const id = key(entry)
	if (selection.value.has(id)) selection.value.delete(id)
	else selection.value.add(id)
	confirmRemoval.value = false
}
async function removeSelected() {
	if (props.disabled || !confirmRemoval.value || !selected.value.length) return
	const entries = [...selected.value]
	await run(async () => {
		await props.adapter.remove(entries)
		selection.value.clear()
		confirmRemoval.value = false
		await load()
	})
}
watch(
	() => props.adapter,
	() => {
		modal.value?.hide()
		preview.value = null
		selection.value.clear()
	},
)
defineExpose({ show })
</script>

<template>
	<NewModal
		ref="modal"
		:header="formatMessage(messages.title)"
		max-width="820px"
		width="820px"
		scrollable
		:disable-close="busy"
	>
		<div class="flex flex-col gap-4">
			<div class="flex flex-wrap gap-2">
				<Button
					:type="tab === 'recovery' ? 'colored' : 'outlined'"
					color="brand"
					:disabled="busy"
					@click="show('recovery')"
					>{{ formatMessage(messages.recovery) }}</Button
				>
				<Button
					:type="tab === 'storage' ? 'colored' : 'outlined'"
					color="brand"
					:disabled="busy"
					@click="show('storage')"
					>{{ formatMessage(messages.storage) }}</Button
				>
				<Button type="outlined" :disabled="busy" @click="run(() => load())">{{
					formatMessage(commonMessages.refreshButton)
				}}</Button>
			</div>
			<p v-if="busy" role="status">{{ formatMessage(messages.loading) }}</p>
			<Admonition v-if="failed" type="warning" role="alert">{{
				formatMessage(messages.error)
			}}</Admonition>
			<Admonition v-if="incomplete" type="warning">{{
				formatMessage(messages.partial)
			}}</Admonition>
			<template v-if="tab === 'recovery'">
				<p class="m-0 text-secondary">{{ formatMessage(messages.restoreHelp) }}</p>
				<p v-if="!busy && !failed && !copies.length">{{ formatMessage(messages.empty) }}</p>
				<div class="flex min-w-0 flex-col gap-2">
					<div
						v-for="copy in copies"
						:key="key(copy)"
						class="flex flex-wrap items-center justify-between gap-3 rounded-xl bg-surface-3 p-3"
					>
						<div class="min-w-0 flex-1 break-all">
							<p class="m-0 font-semibold">{{ copy.path }}</p>
							<p class="mb-0 text-sm text-secondary">
								{{
									formatMessage(messages.origin, {
										source:
											copy.source ?? (adapter.edition === 'java' ? 'java_editor' : 'management'),
									})
								}}
								· {{ date(copy.created) }} · {{ formatBytes(copy.size) }} ·
								{{ formatMessage(messages.state, { state: copy.state }) }}
							</p>
							<span v-if="copy.limited">{{ formatMessage(messages.partial) }}</span>
						</div>
						<Button type="outlined" :disabled="busy" @click="selectCopy(copy)">{{
							formatMessage(messages.preview)
						}}</Button>
					</div>
				</div>
				<Button
					v-if="copies.length < total"
					type="outlined"
					:disabled="busy"
					@click="run(() => load(true))"
					>{{ formatMessage(messages.loadMore) }}</Button
				>
				<section v-if="preview" class="flex flex-col gap-3 rounded-xl bg-surface-4 p-4">
					<p class="m-0 break-all font-semibold">{{ preview.copy.path }}</p>
					<p class="m-0">
						{{ formatMessage(messages.state, { state: preview.copy.state }) }} ·
						{{ formatBytes(preview.copy.size) }}
					</p>
					<pre
						v-if="preview.content !== undefined"
						class="m-0 max-h-48 overflow-auto whitespace-pre-wrap break-all rounded-lg bg-surface-2 p-3"
						>{{ preview.content }}</pre>
					<ul v-if="preview.conflicts.length">
						<li v-for="conflict in preview.conflicts" :key="conflict" class="break-all">
							{{ conflict }}
						</li>
					</ul>
					<Button
						type="colored"
						color="brand"
						:disabled="disabled || busy || !preview.canRestore"
						@click="restore"
						>{{ formatMessage(messages.restore) }}</Button
					>
				</section>
			</template>
			<template v-else>
				<p class="m-0 text-secondary">{{ formatMessage(messages.removeHelp) }}</p>
				<section class="flex flex-col gap-3 rounded-xl bg-surface-3 p-3">
					<p class="m-0 text-secondary">{{ formatMessage(messages.retentionHelp) }}</p>
					<div class="flex flex-wrap items-end gap-3">
						<label class="flex min-w-0 flex-1 flex-col gap-1"
							>{{ formatMessage(messages.retentionDays) }}
							<input
								v-model.number="retentionDays"
								type="number"
								min="1"
								max="3650"
								:disabled="disabled || busy"
								class="retention-input w-24 rounded-lg bg-surface-4 p-2 text-contrast"
							/>
						</label>
						<label class="flex min-w-0 flex-1 flex-col gap-1"
							>{{ formatMessage(messages.retentionKeep) }}
							<input
								v-model.number="retentionKeep"
								type="number"
								min="1"
								max="1000"
								:disabled="disabled || busy"
								class="retention-input w-24 rounded-lg bg-surface-4 p-2 text-contrast"
							/>
						</label>
						<Button
							type="outlined"
							:disabled="disabled || busy || !retentionValid"
							@click="selectOldCopies"
							>{{ formatMessage(messages.retentionSelect) }}</Button
						>
					</div>
				</section>
				<div class="grid grid-cols-2 gap-3 sm:grid-cols-3">
					<div
						v-for="(bytes, category) in categories"
						:key="category"
						class="min-w-0 rounded-xl bg-surface-3 p-3"
					>
						<strong class="break-words">{{
							formatMessage(categoryMessage(String(category)))
						}}</strong>
						<p class="mb-0 text-secondary">{{ formatBytes(bytes) }}</p>
					</div>
				</div>
				<div class="flex min-w-0 flex-col gap-2">
					<Checkbox
						v-for="entry in storage"
						:key="key(entry)"
						class="storage-entry w-full rounded-xl"
						:description="entry.path"
						:model-value="selection.has(key(entry))"
						:disabled="disabled || busy || !entry.removable"
						@update:model-value="toggle(entry)"
					>
						<span class="flex min-w-0 flex-1 flex-col gap-1" aria-hidden="true">
							<span class="break-all font-semibold">{{ entry.path }}</span>
							<span
								class="flex flex-wrap gap-x-3 gap-y-1 text-sm"
								:class="selection.has(key(entry)) ? 'text-contrast' : 'text-secondary'"
							>
								<span>{{ formatMessage(categoryMessage(entry.category)) }}</span>
								<span>{{ formatBytes(entry.size) }}</span>
								<span v-if="entry.limited">{{ formatMessage(messages.partial) }}</span>
							</span>
						</span>
					</Checkbox>
				</div>
				<p class="m-0">{{ formatMessage(messages.selected, { count: selected.length }) }}</p>
				<Admonition v-if="confirmRemoval" type="warning"
					><p>{{ formatMessage(messages.removeHelp) }}</p>
					<ul>
						<li v-for="entry in selected" :key="key(entry)" class="break-all">{{ entry.path }}</li>
					</ul></Admonition
				>
				<Button
					type="colored"
					color="red"
					:disabled="disabled || busy || !selected.length"
					@click="confirmRemoval ? removeSelected() : (confirmRemoval = true)"
					>{{ formatMessage(confirmRemoval ? messages.confirmRemoval : messages.remove) }}</Button
				>
			</template>
		</div>
	</NewModal>
</template>

<style scoped>
.storage-entry {
	background: var(--surface-3);
	padding: 0.75rem;
}

.storage-entry[aria-checked='true'] {
	background: var(--color-brand-highlight);
	color: var(--color-contrast);
}

.storage-entry:deep(> span:first-child) {
	border-radius: 50%;
}

.retention-input {
	box-sizing: border-box;
	border: 1px solid var(--surface-5);
}
</style>
