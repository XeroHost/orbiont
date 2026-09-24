<script setup lang="ts">
import { SearchIcon } from '@modrinth/assets'
import { Button, commonMessages, defineMessages, EmptyState, Input, useVIntl } from '@modrinth/ui'
import { computed, ref } from 'vue'

import OrbiontServerList from '@/components/ui/orbiont/OrbiontServerList.vue'
import { useOrbiontCatalog } from '@/composables/use-orbiont-catalog'

defineOptions({ name: 'ServersPage' })

const { formatMessage } = useVIntl()
const { servers, modpacks, loading, error, refetch } = useOrbiontCatalog()

const search = ref('')
const filteredServers = computed(() => {
	const query = search.value.trim().toLowerCase()
	if (!query) return servers.value
	return servers.value.filter(
		(server) =>
			server.name.toLowerCase().includes(query) ||
			server.tags.some((tag) => tag.toLowerCase().includes(query)),
	)
})

const messages = defineMessages({
	heading: { id: 'app.orbiont.servers.heading', defaultMessage: 'Servers' },
	search: { id: 'app.orbiont.servers.search', defaultMessage: 'Search servers' },
	errorHeading: {
		id: 'app.orbiont.servers.error',
		defaultMessage: "Couldn't load the server list",
	},
	emptyHeading: { id: 'app.orbiont.servers.empty', defaultMessage: 'No servers yet' },
	emptyDescription: {
		id: 'app.orbiont.servers.empty.description',
		defaultMessage: 'Servers will show up here as soon as they are published.',
	},
	noResults: {
		id: 'app.orbiont.servers.no-results',
		defaultMessage: 'No servers match your search',
	},
})
</script>

<template>
	<div class="box-border flex h-full flex-col gap-4 p-6">
		<h1 class="m-0 text-2xl font-bold text-contrast">{{ formatMessage(messages.heading) }}</h1>

		<EmptyState
			v-if="error"
			type="no-connection"
			:heading="formatMessage(messages.errorHeading)"
			:description="error.message"
		>
			<template #actions>
				<Button type="outlined" @click="refetch()">
					{{ formatMessage(commonMessages.retryButton) }}
				</Button>
			</template>
		</EmptyState>

		<template v-else-if="!loading">
			<EmptyState
				v-if="servers.length === 0"
				type="no-documents"
				:heading="formatMessage(messages.emptyHeading)"
				:description="formatMessage(messages.emptyDescription)"
			/>
			<template v-else>
				<Input
					v-model="search"
					:icon="SearchIcon"
					type="text"
					:placeholder="formatMessage(messages.search)"
					clearable
					wrapper-class="max-w-md"
				/>
				<EmptyState
					v-if="filteredServers.length === 0"
					type="no-search-result"
					:heading="formatMessage(messages.noResults)"
				/>
				<OrbiontServerList v-else :servers="filteredServers" :modpacks="modpacks" />
			</template>
		</template>
	</div>
</template>
