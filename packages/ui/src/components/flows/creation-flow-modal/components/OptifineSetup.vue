<template>
	<section class="flex flex-col gap-2">
		<span class="font-semibold text-contrast">{{ formatMessage(messages.label) }}</span>
		<Chips
			v-model="choice"
			:items="['none', 'optifine']"
			:format-label="formatChoice"
			:disabled-items="supported ? [] : ['optifine']"
		/>
		<p class="m-0 text-sm text-secondary">
			{{ formatMessage(supported ? messages.description : messages.vanillaOnly) }}
		</p>
		<div
			v-if="ctx.optifineEnabled.value"
			class="flex flex-col gap-3 rounded-xl border border-solid border-surface-5 bg-surface-2 p-3"
		>
			<div class="flex flex-wrap gap-2">
				<Button type="outlined" :disabled="ctx.optifineBusy.value" @click="download"
					><DownloadIcon />{{ formatMessage(messages.download) }}</Button
				>
				<Button
					type="outlined"
					:disabled="ctx.optifineBusy.value || !ctx.selectedGameVersion.value"
					@click="choose"
					><UploadIcon />{{ formatMessage(messages.select) }}</Button
				>
			</div>
			<span v-if="ctx.optifineBusy.value" class="text-sm text-secondary">{{
				formatMessage(messages.validating)
			}}</span>
			<span v-else-if="ctx.optifineInstaller.value" class="text-sm text-contrast">{{
				formatMessage(messages.selected, {
					version: ctx.optifineInstaller.value.reference.version,
					gameVersion: ctx.optifineInstaller.value.reference.minecraftVersion,
				})
			}}</span>
			<span v-else class="text-sm text-secondary">{{
				formatMessage(messages.required, { gameVersion: ctx.selectedGameVersion.value ?? '' })
			}}</span>
		</div>
	</section>
</template>

<script setup lang="ts">
import { DownloadIcon, UploadIcon } from '@orbiont/assets'
import { computed, watch } from 'vue'

import { Button } from '#ui/components/base/buttons'
import Chips from '#ui/components/base/Chips.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { injectNotificationManager } from '#ui/providers'

import { injectCreationFlowContext } from '../creation-flow-context'

const ctx = injectCreationFlowContext()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const supported = computed(
	() => ctx.hideLoaderChips.value || ctx.selectedLoader.value === 'vanilla',
)
const choice = computed({
	get: () => (ctx.optifineEnabled.value ? 'optifine' : 'none'),
	set: (value) => {
		ctx.optifineEnabled.value = value === 'optifine' && supported.value
	},
})
const messages = defineMessages({
	label: { id: 'creation-flow.optifine.optimization', defaultMessage: 'Optimization' },
	none: { id: 'creation-flow.optifine.none', defaultMessage: 'None' },
	optifine: { id: 'creation-flow.optifine.name', defaultMessage: 'OptiFine' },
	description: {
		id: 'creation-flow.optifine.description',
		defaultMessage:
			'Optional. Select the official installer and the launcher will configure it in this instance.',
	},
	vanillaOnly: {
		id: 'creation-flow.optifine.vanilla-only',
		defaultMessage: 'OptiFine installation is currently available for Vanilla instances.',
	},
	download: { id: 'creation-flow.optifine.download', defaultMessage: 'Official download' },
	select: { id: 'creation-flow.optifine.select', defaultMessage: 'Select JAR' },
	validating: { id: 'creation-flow.optifine.validating', defaultMessage: 'Checking installer…' },
	selected: {
		id: 'creation-flow.optifine.selected',
		defaultMessage: 'OptiFine {version} · Minecraft {gameVersion}',
	},
	required: {
		id: 'creation-flow.optifine.required',
		defaultMessage: 'Select the official OptiFine JAR for Minecraft {gameVersion} to continue.',
	},
})
function formatChoice(value: string) {
	return formatMessage(value === 'optifine' ? messages.optifine : messages.none)
}
let request = 0
watch([ctx.selectedGameVersion, ctx.selectedLoader, ctx.hideLoaderChips], () => {
	request++
	ctx.optifineInstaller.value = null
	ctx.optifineBusy.value = false
	if (!supported.value) ctx.optifineEnabled.value = false
})
async function download() {
	try {
		await ctx.openOptifineDownloads?.()
	} catch (error) {
		handleError(error as Error)
	}
}
async function choose() {
	const game = ctx.selectedGameVersion.value
	if (!game || !supported.value) return
	const ticket = ++request
	ctx.optifineBusy.value = true
	try {
		const selected = await ctx.pickOptifineInstaller?.(game)
		if (ticket === request && selected) ctx.optifineInstaller.value = selected
	} catch (error) {
		if (ticket === request) handleError(error as Error)
	} finally {
		if (ticket === request) ctx.optifineBusy.value = false
	}
}
</script>
