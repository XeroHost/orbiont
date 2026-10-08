<script setup lang="ts">
import { Admonition, AnimatedIcon, Button, useVIntl } from '@orbiont/ui'

import type { BedrockStatus } from '@/helpers/bedrock'
import { bedrockMessages } from '@/helpers/bedrock-messages'
import { bedrockDiagnosticReport } from '@/helpers/bedrock-operations'
import { managementMessages as messages } from '@/helpers/management-messages'

defineProps<{ status: BedrockStatus; busy: boolean; refreshing: boolean; failed: boolean }>()
const emit = defineEmits<{ launcher: []; store: []; refresh: [] }>()
const { formatMessage } = useVIntl()
function exportReport(status: BedrockStatus) {
	const blob = new Blob([JSON.stringify(bedrockDiagnosticReport(status), null, 2)], {
		type: 'application/json',
	})
	const url = URL.createObjectURL(blob)
	const link = document.createElement('a')
	link.href = url
	link.download = 'bedrock-diagnostic.json'
	link.click()
	setTimeout(() => URL.revokeObjectURL(url), 1000)
}
</script>

<template>
	<section class="flex flex-col gap-3 rounded-xl bg-surface-3 p-5">
		<h2 class="m-0 text-xl font-semibold text-contrast">
			{{ formatMessage(messages.diagnostics) }}
		</h2>
		<Admonition v-if="failed" type="warning" role="alert">{{
			formatMessage(bedrockMessages.detectionError)
		}}</Admonition>
		<p class="m-0" role="status">
			{{ formatMessage(messages.launchState, { state: status.launch?.state ?? 'idle' }) }} ·
			{{
				formatMessage(messages.launchTime, {
					seconds: Math.floor((status.launch?.elapsed_ms ?? 0) / 1000),
				})
			}}
		</p>
		<p class="m-0 text-secondary">{{ formatMessage(messages.repairHelp) }}</p>
		<div class="flex flex-wrap gap-2">
			<Button
				type="outlined"
				:disabled="busy || !status.launcher?.can_launch"
				@click="emit('launcher')"
				>{{ formatMessage(bedrockMessages.launcher) }}</Button
			>
			<Button type="outlined" :disabled="busy" @click="emit('store')">{{
				formatMessage(bedrockMessages.store)
			}}</Button>
			<Button type="outlined" :disabled="busy" :loading="refreshing" @click="emit('refresh')">
				<AnimatedIcon name="refresh" :class="{ 'motion-safe:animate-spin': refreshing }" />{{
					formatMessage(bedrockMessages.refresh)
				}}
			</Button>
			<Button type="outlined" @click="exportReport(status)">{{
				formatMessage(messages.export)
			}}</Button>
		</div>
	</section>
</template>
