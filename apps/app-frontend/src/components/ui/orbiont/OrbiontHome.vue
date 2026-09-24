<script setup lang="ts">
import { PlayIcon, SearchIcon } from '@modrinth/assets'
import { Avatar, Button, ButtonLink } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import OrbiontServerStatus from '@/components/ui/orbiont/OrbiontServerStatus.vue'
import { handleSevereError } from '@/composables/use-error.js'
import type { Modpack, OrbiontServer } from '@/helpers/orbiont'
import { getModpacks, getServers } from '@/helpers/orbiont'
import { injectAppEvents } from '@/providers/app-events'
import { playOrbiontServer } from '@/providers/orbiont-play'

const appEvents = injectAppEvents()

const modpacksQuery = useQuery({
	queryKey: ['orbiont', 'modpacks'],
	queryFn: getModpacks,
	staleTime: 60_000,
})
const modpacks = computed(() => modpacksQuery.data.value ?? [])

const serversQuery = useQuery({
	queryKey: ['orbiont', 'servers'],
	queryFn: getServers,
	staleTime: 60_000,
})
const servers = computed(() =>
	(serversQuery.data.value ?? []).slice().sort((a, b) => Number(b.featured) - Number(a.featured)),
)

function modpackFor(server: OrbiontServer): Modpack | null {
	return modpacks.value.find((pack) => pack.id === server.modpackId) ?? null
}

const joiningServerId = ref<string | null>(null)

async function onPlay(server: OrbiontServer) {
	if (joiningServerId.value) return
	joiningServerId.value = server.id
	try {
		await playOrbiontServer(server, modpackFor(server), appEvents)
	} catch (err) {
		handleSevereError(err, { serverId: server.id })
	} finally {
		joiningServerId.value = null
	}
}
</script>

<template>
	<div v-if="modpacks.length > 0 || servers.length > 0" class="flex flex-col gap-4">
		<div class="flex justify-end">
			<ButtonLink type="outlined" :to="{ path: '/orbiont/search' }">
				<SearchIcon />
				Search modpacks
			</ButtonLink>
		</div>

		<section v-if="modpacks.length > 0" class="flex flex-col gap-2">
			<h2 class="m-0 text-lg font-extrabold text-contrast">Featured modpacks</h2>
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
			<h2 class="m-0 text-lg font-extrabold text-contrast">Servers</h2>
			<div class="flex flex-col gap-2">
				<div
					v-for="server in servers"
					:key="server.id"
					class="flex items-center gap-3 rounded-2xl bg-bg-raised p-3"
				>
					<Avatar :src="server.icon" size="48px" class="rounded-xl" />
					<div class="flex min-w-0 flex-1 flex-col gap-0.5">
						<div class="flex items-center gap-2">
							<span class="truncate font-bold text-contrast">{{ server.name }}</span>
							<span
								v-if="server.featured"
								class="rounded-full bg-brand-highlight px-2 py-0.5 text-xs font-bold text-brand"
							>
								Featured
							</span>
						</div>
						<OrbiontServerStatus :address="`${server.host}:${server.port}`" />
					</div>
					<Button
						type="colored"
						color="brand"
						:disabled="joiningServerId !== null"
						:aria-busy="joiningServerId === server.id"
						@click="onPlay(server)"
					>
						<PlayIcon />
						{{ joiningServerId === server.id ? 'Starting…' : 'Play' }}
					</Button>
				</div>
			</div>
		</section>
	</div>
</template>
