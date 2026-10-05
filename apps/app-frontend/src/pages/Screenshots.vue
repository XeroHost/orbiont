<script setup lang="ts">
import { ImageIcon } from '@orbiont/assets'
import { defineMessages, useVIntl } from '@orbiont/ui'
import { computed, onActivated } from 'vue'
import { useRoute } from 'vue-router'

import ScreenshotsPage from '@/components/ui/screenshots-page/index.vue'
import { useRootBreadcrumb } from '@/providers/breadcrumbs'

defineOptions({ name: 'ScreenshotsPage' })

const { formatMessage } = useVIntl()
const route = useRoute()
const bedrock = computed(() => route.query.edition === 'bedrock')
const messages = defineMessages({
	screenshots: { id: 'app.screenshots.heading', defaultMessage: 'Screenshots' },
})
const breadcrumb = useRootBreadcrumb({
	slot: 'root',
	id: 'screenshots',
	label: formatMessage(messages.screenshots),
	to: () => (bedrock.value ? '/screenshots?edition=bedrock' : '/screenshots'),
	visual: { type: 'icon', component: ImageIcon },
})
onActivated(breadcrumb.reset)
</script>

<template>
	<div class="box-border h-full p-6">
		<ScreenshotsPage :key="bedrock ? 'bedrock' : 'java'" :bedrock="bedrock" show-heading />
	</div>
</template>
