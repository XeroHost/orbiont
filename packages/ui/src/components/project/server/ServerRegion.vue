<script setup lang="ts">
import { computed } from 'vue'

import { defineMessage, useVIntl } from '../../../composables'
import { SERVER_REGIONS } from '../../../utils'
import { TagItem } from '../../base'

const { region } = defineProps<{
	region: string
}>()

const { formatMessage } = useVIntl()

const tooltip = defineMessage({
	id: 'project.server.region.tooltip',
	defaultMessage: 'Server hosted in {regionName}',
})

const regionName = computed(() => {
	const name = SERVER_REGIONS[region as keyof typeof SERVER_REGIONS]
	if (name) return formatMessage(name)

	return region
})
</script>
<template>
	<TagItem v-tooltip="formatMessage(tooltip, { regionName })">{{ regionName }}</TagItem>
</template>
