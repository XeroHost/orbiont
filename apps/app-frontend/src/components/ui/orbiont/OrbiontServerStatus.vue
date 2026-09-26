<script setup lang="ts">
import { UsersIcon } from '@orbiont/assets'
import { defineMessages, useVIntl } from '@orbiont/ui'
import { onMounted, ref } from 'vue'

import { get_server_status } from '@/helpers/worlds'

const props = defineProps<{ address: string }>()

const { formatMessage } = useVIntl()

const status = ref<'loading' | 'online' | 'offline'>('loading')
const players = ref<number | null>(null)
const ping = ref<number | null>(null)

onMounted(async () => {
	try {
		const result = await get_server_status(props.address, null)
		status.value = 'online'
		players.value = result.players?.online ?? null
		ping.value = result.ping ?? null
	} catch {
		// The server not answering is a normal, expected state here (it
		// might just be offline) — not something to surface as an app error.
		status.value = 'offline'
	}
})

const messages = defineMessages({
	checking: { id: 'app.orbiont.server-status.checking', defaultMessage: 'Checking…' },
	offline: { id: 'app.orbiont.server-status.offline', defaultMessage: 'Offline' },
})
</script>

<template>
	<span v-if="status === 'loading'" class="text-sm text-secondary">
		{{ formatMessage(messages.checking) }}
	</span>
	<span v-else-if="status === 'offline'" class="text-sm text-red">
		{{ formatMessage(messages.offline) }}
	</span>
	<span v-else class="flex items-center gap-1 text-sm text-secondary">
		<UsersIcon class="h-4 w-4" />
		{{ players ?? '?' }}
		<span v-if="ping != null">· {{ ping }}ms</span>
	</span>
</template>
