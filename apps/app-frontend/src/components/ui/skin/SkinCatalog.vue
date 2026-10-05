<script setup lang="ts">
import {
	ChevronLeftIcon,
	ChevronRightIcon,
	SaveIcon,
	SearchIcon,
	SpinnerIcon,
} from '@orbiont/assets'
import {
	Button,
	Combobox,
	commonMessages,
	defineMessages,
	IconButton,
	Input,
	useVIntl,
} from '@orbiont/ui'
import { useQuery } from '@tanstack/vue-query'
import { useElementSize, watchDebounced } from '@vueuse/core'
import { computed, ref, useTemplateRef, watch } from 'vue'

import { catalogSkin, getSkinCatalog, type RemoteSkinProvider } from '@/helpers/skin-catalog'
import { getSkinGridColumns } from '@/helpers/skin-grid'
import type { Skin } from '@/helpers/skins'

import BakedSkinButton from './BakedSkinButton.vue'

const props = defineProps<{
	provider: RemoteSkinProvider
	selectedSkin: Skin | null
	readOnly: boolean
	saving: boolean
}>()
const emit = defineEmits<{ select: [skin: Skin]; save: [skin: Skin] }>()
const { formatMessage } = useVIntl()
const messages = defineMessages({
	search: { id: 'app.skins.catalog.search', defaultMessage: 'Search skins…' },
	searchButton: { id: 'app.skins.catalog.search-button', defaultMessage: 'Search' },
	mineSkinNote: {
		id: 'app.skins.catalog.mineskin-search-note',
		defaultMessage: 'Search the entire public MineSkin gallery.',
	},
	model: { id: 'app.skins.catalog.model', defaultMessage: 'Model' },
	allModels: { id: 'app.skins.catalog.all-models', defaultMessage: 'All models' },
	classic: { id: 'app.skins.catalog.classic', defaultMessage: 'Classic' },
	slim: { id: 'app.skins.catalog.slim', defaultMessage: 'Slim' },
	sort: { id: 'app.skins.catalog.sort', defaultMessage: 'Sort by' },
	recent: { id: 'app.skins.catalog.recent', defaultMessage: 'Recent' },
	popular: { id: 'app.skins.catalog.popular', defaultMessage: 'Popular' },
	trending: { id: 'app.skins.catalog.trending', defaultMessage: 'Trending' },
	tag: { id: 'app.skins.catalog.tag', defaultMessage: 'Tag (optional)' },
	loading: { id: 'app.skins.catalog.loading', defaultMessage: 'Loading skins…' },
	empty: { id: 'app.skins.catalog.empty', defaultMessage: 'No skins found. Try another search.' },
	error: {
		id: 'app.skins.catalog.error',
		defaultMessage: 'This skin provider is unavailable. Please try again later.',
	},
	unconfigured: {
		id: 'app.skins.catalog.unconfigured',
		defaultMessage: 'This skin provider is not configured yet.',
	},
	rateLimit: {
		id: 'app.skins.catalog.rate-limit',
		defaultMessage: 'The provider has reached its request limit. Please wait before trying again.',
	},
	retry: { id: 'app.skins.catalog.retry', defaultMessage: 'Try again' },
	previous: { id: 'app.skins.catalog.previous', defaultMessage: 'Previous page' },
	next: { id: 'app.skins.catalog.next', defaultMessage: 'Next page' },
	page: { id: 'app.skins.catalog.page', defaultMessage: 'Page {page}' },
})
const search = ref('')
const submittedSearch = ref('')
const tag = ref('')
const submittedTag = ref('')
const model = ref('')
const sort = ref('recent')
const catalogContainer = useTemplateRef<HTMLElement>('catalogContainer')
const { width: catalogWidth } = useElementSize(catalogContainer)
const gridStyle = computed(() => ({
	gridTemplateColumns: `repeat(${getSkinGridColumns(catalogWidth.value)}, minmax(0, 1fr))`,
}))
const modelOptions = computed(() => [
	{ value: '', label: formatMessage(messages.allModels) },
	{ value: 'classic', label: formatMessage(messages.classic) },
	{ value: 'slim', label: formatMessage(messages.slim) },
])
const sortOptions = computed(() => [
	{ value: 'recent', label: formatMessage(messages.recent) },
	{ value: 'popular', label: formatMessage(messages.popular) },
	{ value: 'trending', label: formatMessage(messages.trending) },
])
const page = ref(1)
const cursors = ref<string[]>([''])
const query = useQuery(
	computed(() => {
		const provider = props.provider
		const filters =
			provider === 'mcstat'
				? {
						page: page.value,
						search: submittedSearch.value,
						tag: submittedTag.value,
						model: model.value,
						sort: sort.value,
					}
				: { after: cursors.value[page.value - 1] || '', search: submittedSearch.value }
		return {
			queryKey: ['skin-catalog', provider, filters],
			queryFn: () => getSkinCatalog(provider, filters),
			staleTime: provider === 'mineskin' && submittedSearch.value ? 5 * 60_000 : 60_000,
			gcTime: 5 * 60_000,
			retry: false,
			refetchOnWindowFocus: false,
		}
	}),
)
const entries = computed(() =>
	(query.data.value?.skins || []).map((item) => ({
		...item,
		skin: catalogSkin(item, props.provider),
	})),
)
const errorMessage = computed(() => {
	const error = String(query.error.value)
	return formatMessage(
		error.includes(': 429')
			? messages.rateLimit
			: error.includes(': 503')
				? messages.unconfigured
				: messages.error,
	)
})

function searchCatalog() {
	const nextSearch = search.value.trim().slice(0, 80)
	if (nextSearch !== submittedSearch.value) cursors.value = ['']
	submittedSearch.value = nextSearch
	submittedTag.value = tag.value.trim().slice(0, 80)
	page.value = 1
}
watchDebounced(
	search,
	() => {
		if (
			props.provider === 'mineskin' &&
			search.value.trim().slice(0, 80) !== submittedSearch.value
		) {
			searchCatalog()
		}
	},
	{ debounce: 500 },
)
function nextPage() {
	const next = query.data.value?.next
	if (!next) return
	if (props.provider === 'mineskin') cursors.value[page.value] = next
	page.value++
}
watch([model, sort], () => {
	page.value = 1
})
</script>

<template>
	<section
		ref="catalogContainer"
		class="flex min-w-0 flex-col gap-4"
		:aria-label="provider === 'mcstat' ? 'MCStat' : 'MineSkin'"
	>
		<form class="flex flex-col gap-3" @submit.prevent="searchCatalog">
			<div class="flex items-center gap-2">
				<Input
					v-model="search"
					:icon="SearchIcon"
					type="text"
					autocomplete="off"
					:maxlength="80"
					wrapper-class="min-w-0 flex-1"
					size="large"
					clearable
					:aria-label="formatMessage(messages.search)"
					:placeholder="formatMessage(messages.search)"
					@clear="searchCatalog"
				/>
				<Button size="lg" native-type="submit" :disabled="query.isFetching.value">
					<SearchIcon />{{ formatMessage(messages.searchButton) }}
				</Button>
			</div>
			<div v-if="provider === 'mcstat'" class="flex flex-wrap items-center gap-3">
				<Combobox
					v-model="model"
					:options="modelOptions"
					trigger-type="base"
					class="!w-[17rem] max-w-full"
				>
					<template #prefix>
						<span class="font-semibold text-primary">{{ formatMessage(messages.model) }}:</span>
					</template>
				</Combobox>
				<Combobox
					v-model="sort"
					:options="sortOptions"
					trigger-type="base"
					class="!w-[16rem] max-w-full"
				>
					<template #prefix>
						<span class="font-semibold text-primary">{{ formatMessage(messages.sort) }}:</span>
					</template>
				</Combobox>
				<Input
					v-model="tag"
					:maxlength="80"
					wrapper-class="min-w-[10rem] flex-1"
					:aria-label="formatMessage(messages.tag)"
					:placeholder="formatMessage(messages.tag)"
				/>
			</div>
			<p v-else class="m-0 text-sm text-secondary">{{ formatMessage(messages.mineSkinNote) }}</p>
		</form>
		<div
			v-if="query.isFetching.value"
			class="flex items-center justify-center gap-2 rounded-xl bg-surface-2 p-8"
			role="status"
		>
			<SpinnerIcon class="size-5 animate-spin" />{{ formatMessage(messages.loading) }}
		</div>
		<div
			v-else-if="query.isError.value"
			class="flex flex-col items-center gap-3 rounded-xl bg-surface-2 p-8"
			role="status"
		>
			<p class="m-0">{{ errorMessage }}</p>
			<Button type="outlined" @click="query.refetch()">{{ formatMessage(messages.retry) }}</Button>
		</div>
		<template v-else>
			<p v-if="!entries.length" class="rounded-xl bg-surface-2 p-8 text-center">
				{{ formatMessage(messages.empty) }}
			</p>
			<div class="grid w-full gap-3" :style="gridStyle">
				<BakedSkinButton
					v-for="entry in entries"
					:key="entry.id"
					class="box-border aspect-[31/40] w-full min-w-0 rounded-[20px]"
					:skin="entry.skin"
					:capes="[]"
					:tooltip="entry.skin.name"
					:selected="selectedSkin?.texture_key === entry.skin.texture_key"
					@select="emit('select', entry.skin)"
				>
					<template v-if="!readOnly" #overlay-buttons>
						<Button
							type="colored"
							color="brand"
							class="pointer-events-auto"
							:disabled="saving"
							@click.stop="emit('save', entry.skin)"
						>
							<SaveIcon />{{ formatMessage(commonMessages.saveButton) }}
						</Button>
					</template>
				</BakedSkinButton>
			</div>
		</template>
		<div class="flex items-center justify-center gap-3">
			<IconButton
				:disabled="page === 1 || query.isFetching.value"
				:label="formatMessage(messages.previous)"
				@click="page--"
				><ChevronLeftIcon
			/></IconButton>
			<span class="text-sm text-secondary">{{ formatMessage(messages.page, { page }) }}</span>
			<IconButton
				:disabled="!query.data.value?.next || query.isFetching.value || query.isError.value"
				:label="formatMessage(messages.next)"
				@click="nextPage"
				><ChevronRightIcon
			/></IconButton>
		</div>
	</section>
</template>
