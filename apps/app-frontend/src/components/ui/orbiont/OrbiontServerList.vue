<script setup lang="ts">
import { PlayIcon } from '@orbiont/assets'
import { Avatar, Button, defineMessages, useVIntl } from '@orbiont/ui'
import { ref } from 'vue'

import OrbiontServerStatus from '@/components/ui/orbiont/OrbiontServerStatus.vue'
import { handleSevereError } from '@/composables/use-error.js'
import type { Modpack, OrbiontServer } from '@/helpers/orbiont'
import { injectAppEvents } from '@/providers/app-events'
import { playOrbiontServer } from '@/providers/orbiont-play'

const props = defineProps<{
	servers: OrbiontServer[]
	modpacks: Modpack[]
}>()

const { formatMessage } = useVIntl()
const appEvents = injectAppEvents()

const joiningServerId = ref<string | null>(null)

function modpackFor(server: OrbiontServer): Modpack | null {
	return props.modpacks.find((pack) => pack.id === server.modpackId) ?? null
}

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

const messages = defineMessages({
	featured: { id: 'app.orbiont.servers.featured', defaultMessage: 'Featured' },
	play: { id: 'app.orbiont.servers.play', defaultMessage: 'Play' },
	starting: { id: 'app.orbiont.servers.starting', defaultMessage: 'Starting…' },
})
</script>

<template>
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
						{{ formatMessage(messages.featured) }}
					</span>
				</div>
				<div class="flex flex-wrap items-center gap-2">
					<OrbiontServerStatus :address="`${server.host}:${server.port}`" />
					<span
						v-for="tag in server.tags"
						:key="tag"
						class="rounded-full bg-button-bg px-2 py-0.5 text-xs text-secondary"
					>
						{{ tag }}
					</span>
				</div>
			</div>
			<Button
				type="colored"
				color="brand"
				:disabled="joiningServerId !== null"
				:aria-busy="joiningServerId === server.id"
				@click="onPlay(server)"
			>
				<PlayIcon />
				{{ formatMessage(joiningServerId === server.id ? messages.starting : messages.play) }}
			</Button>
		</div>
	</div>
</template>
