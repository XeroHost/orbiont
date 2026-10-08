<script setup lang="ts">
import { FolderOpenIcon } from '@orbiont/assets'
import {
	commonMessages,
	ContentCardLayout,
	type ContentItem,
	NewModal,
	provideContentManager,
	useVIntl,
} from '@orbiont/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

import type { BedrockItem } from '@/helpers/bedrock'
import { bedrockMessages as messages } from '@/helpers/bedrock-messages'
import { managementMessages } from '@/helpers/management-messages'

const props = defineProps<{
	items: BedrockItem[]
	worlds: BedrockItem[]
	busy: boolean
	confirmDelete: (items: BedrockItem[]) => Promise<boolean>
	deleteItem: (item: BedrockItem) => Promise<void>
	deleteItems: (items: BedrockItem[]) => Promise<void>
}>()
const emit = defineEmits<{ openFolder: [rootId: string, path: string]; import: []; refresh: [] }>()
const { formatMessage } = useVIntl()
const router = useRouter()
const details = ref<InstanceType<typeof NewModal>>()
const selectedPack = ref<BedrockItem | null>(null)
const kinds = {
	resource_pack: messages.resourcePack,
	skin_pack: messages.skinPack,
	behavior_pack: messages.addons,
}
const items = computed<ContentItem[]>(() =>
	props.items
		.filter((item) => item.kind in kinds)
		.map((item) => {
			const id = `${item.root_id}/${item.path}`
			const kind = item.kind as keyof typeof kinds
			const parts = item.path.split('/')
			const world =
				parts[0] === 'minecraftWorlds'
					? props.worlds.find(
							(world) =>
								world.root_id === item.root_id && world.path === `minecraftWorlds/${parts[1]}`,
						)
					: undefined
			return {
				id,
				project: {
					id,
					slug: id,
					title: item.name,
					icon_url: item.icon_path ? convertFileSrc(item.icon_path) : '',
				},
				subtitle: world
					? `${formatMessage(kinds[kind])} · ${formatMessage(messages.worldScope, { name: world.name })}`
					: formatMessage(kinds[kind]),
				version: {
					id,
					version_number: item.pack_version?.join('.') ?? item.version ?? '—',
					file_name: item.path.split('/').at(-1) ?? item.name,
				},
				file_name: item.path.split('/').at(-1) ?? item.name,
				file_path: item.path,
				project_type:
					kind === 'behavior_pack' ? 'mod' : kind === 'skin_pack' ? 'bedrock_skin' : 'resourcepack',
				has_update: false,
				update_version_id: null,
				hideToggle: true,
				hideDelete: false,
				hideSwitchVersion: true,
			}
		}),
)
async function unavailable(): Promise<never> {
	throw new Error('Bedrock content is managed in Minecraft')
}
provideContentManager({
	items,
	loading: ref(false),
	error: ref(null),
	managedContent: ref(null),
	isPackLocked: ref(false),
	isBusy: computed(() => props.busy),
	contentTypeLabel: computed(() => formatMessage(commonMessages.projectLabel)),
	getTypeLabel: (type) =>
		type === 'mod'
			? formatMessage(messages.addons)
			: type === 'bedrock_skin'
				? formatMessage(messages.skinPack)
				: undefined,
	toggleEnabled: unavailable,
	deleteItem: async (item) => {
		const source = props.items.find(
			(candidate) => `${candidate.root_id}/${candidate.path}` === item.id,
		)
		if (source) await props.deleteItem(source)
	},
	bulkDeleteItems: async (items) => {
		await props.deleteItems(
			props.items.filter((candidate) =>
				items.some((item) => item.id === `${candidate.root_id}/${candidate.path}`),
			),
		)
	},
	confirmDeleteItems: (items) =>
		props.confirmDelete(
			props.items.filter((candidate) =>
				items.some((item) => item.id === `${candidate.root_id}/${candidate.path}`),
			),
		),
	canToggleItem: () => false,
	canDeleteItem: () => true,
	hasUpdateSupport: false,
	refresh: async () => {
		emit('refresh')
	},
	browse: () => {
		void router.push('/browse/mod?edition=bedrock&src=curseforge')
	},
	uploadFiles: () => emit('import'),
	mapToTableItem: (item) => item,
	getOverflowOptions: (item) => [
		{
			id: 'pack-details',
			label: formatMessage(managementMessages.packDetails),
			icon: FolderOpenIcon,
			action: () => {
				selectedPack.value =
					props.items.find((candidate) => `${candidate.root_id}/${candidate.path}` === item.id) ??
					null
				details.value?.show()
			},
		},
		{
			id: 'open-folder',
			label: formatMessage(messages.openFolder),
			icon: FolderOpenIcon,
			action: () => {
				const source = props.items.find(
					(candidate) => `${candidate.root_id}/${candidate.path}` === item.id,
				)
				if (source) emit('openFolder', source.root_id, source.path)
			},
		},
	],
	filterPersistKey: 'bedrock-content',
})
</script>

<template>
	<ContentCardLayout :bottom-padding="false" />
	<NewModal ref="details" :header="formatMessage(managementMessages.packDetails)" max-width="720px">
		<div v-if="selectedPack" class="flex flex-col gap-4">
			<h3 class="m-0 break-all text-contrast">{{ selectedPack.name }}</h3>
			<p class="m-0">
				{{ selectedPack.pack_version?.join('.') ?? selectedPack.version ?? '�' }} �
				{{
					formatMessage(managementMessages.packState, {
						active: String(selectedPack.active ?? false),
					})
				}}
			</p>
			<p class="m-0 break-all text-secondary">{{ selectedPack.pack_id }}</p>
			<section>
				<h4>{{ formatMessage(managementMessages.dependencies) }}</h4>
				<p v-if="!selectedPack.dependencies?.length">
					{{ formatMessage(managementMessages.none) }}
				</p>
				<ul v-else>
					<li
						v-for="dependency in selectedPack.dependencies"
						:key="dependency.pack_id"
						class="break-all"
					>
						{{ dependency.pack_id }} � {{ dependency.version.join('.') }}
					</li>
				</ul>
			</section>
			<section>
				<h4>{{ formatMessage(managementMessages.activations) }}</h4>
				<p v-if="!selectedPack.activations?.length">{{ formatMessage(managementMessages.none) }}</p>
				<ul v-else>
					<li
						v-for="activation in selectedPack.activations"
						:key="`${activation.root_id}/${activation.world_path}`"
						class="break-all"
					>
						{{ activation.world_path }} � {{ activation.version.join('.') }}
					</li>
				</ul>
			</section>
		</div>
	</NewModal>
</template>
