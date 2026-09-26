<script setup lang="ts">
import { SearchIcon } from '@orbiont/assets'
import { Avatar, ButtonLink, defineMessages, useVIntl } from '@orbiont/ui'
import { computed } from 'vue'

import OrbiontServerList from '@/components/ui/orbiont/OrbiontServerList.vue'
import { useOrbiontCatalog } from '@/composables/use-orbiont-catalog'

const HOME_SERVER_LIMIT = 3

const { formatMessage } = useVIntl()
const { modpacks, servers } = useOrbiontCatalog()

// Featured servers first (the list is already sorted that way).
const homeServers = computed(() => servers.value.slice(0, HOME_SERVER_LIMIT))

const messages = defineMessages({
	searchModpacks: { id: 'app.orbiont.home.search-modpacks', defaultMessage: 'Search modpacks' },
	featuredModpacks: {
		id: 'app.orbiont.home.featured-modpacks',
		defaultMessage: 'Featured modpacks',
	},
	servers: { id: 'app.orbiont.home.servers', defaultMessage: 'Servers' },
	seeAllServers: { id: 'app.orbiont.home.see-all-servers', defaultMessage: 'See all' },
})
</script>

<template>
	<div v-if="modpacks.length > 0 || servers.length > 0" class="flex flex-col gap-4">
		<div class="flex justify-end">
			<ButtonLink type="outlined" :to="{ path: '/browse/modpack' }">
				<SearchIcon />
				{{ formatMessage(messages.searchModpacks) }}
			</ButtonLink>
		</div>

		<section v-if="modpacks.length > 0" class="flex flex-col gap-2">
			<h2 class="m-0 text-lg font-extrabold text-contrast">
				{{ formatMessage(messages.featuredModpacks) }}
			</h2>
			<div class="flex gap-3 overflow-x-auto pb-1">
				<div
					v-for="pack in modpacks"
					:key="pack.id"
					class="flex w-48 shrink-0 flex-col gap-2 rounded-2xl bg-bg-raised p-3"
				>
					<Avatar :src="pack.icon" size="96px" class="rounded-xl" />
					<div class="flex flex-col gap-0.5">
						<span class="truncate font-bold text-contrast">{{ pack.name }}</span>
						<span class="truncate text-sm text-secondary"
							>{{ pack.gameVersion }} · {{ pack.loader }}</span
						>
					</div>
				</div>
			</div>
		</section>

		<section v-if="servers.length > 0" class="flex flex-col gap-2">
			<div class="flex items-center justify-between gap-2">
				<h2 class="m-0 text-lg font-extrabold text-contrast">
					{{ formatMessage(messages.servers) }}
				</h2>
				<ButtonLink
					v-if="servers.length > HOME_SERVER_LIMIT"
					type="transparent"
					:to="{ path: '/servers' }"
				>
					{{ formatMessage(messages.seeAllServers) }}
				</ButtonLink>
			</div>
			<OrbiontServerList :servers="homeServers" :modpacks="modpacks" />
		</section>
	</div>
</template>
