<script setup lang="ts">
import { SearchIcon } from '@modrinth/assets'
import { Button, DropdownSelect, Input } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'

import OrbiontSearchResultCard from '@/components/ui/orbiont/OrbiontSearchResultCard.vue'
import { handleSevereError } from '@/composables/use-error.js'
import type { CatalogSource } from '@/helpers/orbiont'
import { searchCatalog } from '@/helpers/orbiont'

defineOptions({
	name: 'OrbiontSearchPage',
})

const PAGE_SIZE = 20
const SOURCE_OPTIONS: Array<CatalogSource | 'all'> = ['all', 'xerohost', 'curseforge', 'modrinth']
const SOURCE_LABELS: Record<CatalogSource | 'all', string> = {
	all: 'All sources',
	xerohost: 'XeroHost',
	curseforge: 'CurseForge',
	modrinth: 'Modrinth',
}

const queryInput = ref('')
const debouncedQuery = ref('')
const source = ref<CatalogSource | 'all'>('all')
const index = ref(0)

let debounceHandle: ReturnType<typeof setTimeout> | undefined
watch(queryInput, (value) => {
	clearTimeout(debounceHandle)
	debounceHandle = setTimeout(() => {
		debouncedQuery.value = value
		index.value = 0
	}, 300)
})

watch(source, () => {
	index.value = 0
})

const searchQuery = useQuery({
	queryKey: computed(() => ['orbiont', 'search', debouncedQuery.value, source.value, index.value]),
	queryFn: () =>
		searchCatalog({
			query: debouncedQuery.value || undefined,
			source: source.value,
			index: index.value,
			pageSize: PAGE_SIZE,
		}),
})

const results = computed(() => searchQuery.data.value?.results ?? [])
const errors = computed(() => searchQuery.data.value?.errors ?? [])
const totalCount = computed(() => searchQuery.data.value?.totalCount ?? null)

const hasPrevPage = computed(() => index.value > 0)
const hasNextPage = computed(() => {
	if (totalCount.value != null) return index.value + PAGE_SIZE < totalCount.value
	return results.value.length === PAGE_SIZE
})

function prevPage() {
	if (hasPrevPage.value) index.value = Math.max(0, index.value - PAGE_SIZE)
}

function nextPage() {
	if (hasNextPage.value) index.value += PAGE_SIZE
}

function onResultError(err: unknown) {
	handleSevereError(err)
}
</script>

<template>
	<div class="flex flex-col gap-4 p-6">
		<h1 class="m-0 text-2xl font-extrabold text-contrast">Search modpacks</h1>

		<div class="flex flex-wrap gap-2">
			<Input
				v-model="queryInput"
				:icon="SearchIcon"
				placeholder="Search modpacks…"
				class="min-w-64 flex-1"
			/>
			<DropdownSelect
				v-model="source"
				name="orbiont-search-source"
				:options="SOURCE_OPTIONS"
				:display-name="(option: CatalogSource | 'all') => SOURCE_LABELS[option]"
			/>
		</div>

		<div v-if="errors.length > 0" class="flex flex-col gap-1">
			<span v-for="err in errors" :key="err.source" class="text-sm text-red">
				{{ SOURCE_LABELS[err.source] }}: {{ err.message }}
			</span>
		</div>

		<span v-if="searchQuery.isFetching.value" class="text-sm text-secondary">Searching…</span>
		<span v-else-if="results.length === 0" class="text-sm text-secondary">No results.</span>

		<div class="flex flex-col gap-2">
			<OrbiontSearchResultCard
				v-for="result in results"
				:key="result.id"
				:result="result"
				@error="onResultError"
			/>
		</div>

		<div v-if="results.length > 0" class="flex justify-center gap-2">
			<Button :disabled="!hasPrevPage" @click="prevPage">Previous</Button>
			<Button :disabled="!hasNextPage" @click="nextPage">Next</Button>
		</div>
	</div>
</template>
