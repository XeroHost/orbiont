<script setup lang="ts">
import { LeftArrowIcon, RightArrowIcon, XIcon } from '@orbiont/assets'
import { computed } from 'vue'

import IconButton from '#ui/components/base/buttons/IconButton.vue'
import { useVIntl } from '#ui/composables/i18n'
import { commonMessages } from '#ui/utils/common-messages'

import { imageViewerEditorMessages as messages } from './image-viewer-editor-messages'
import type { ImageViewerEditorItem } from './types'

const props = defineProps<{
	item: ImageViewerEditorItem
	index: number
	count: number
}>()
const emit = defineEmits<{ close: []; previous: []; next: [] }>()
const { formatMessage } = useVIntl()
const embedUrl = computed(() => {
	if (!props.item.youtubeVideoId || !/^[A-Za-z0-9_-]{11}$/.test(props.item.youtubeVideoId))
		return undefined
	const url = new URL(`https://www.youtube-nocookie.com/embed/${props.item.youtubeVideoId}`)
	url.searchParams.set('rel', '0')
	if (['http:', 'https:'].includes(window.location.protocol)) {
		url.searchParams.set('origin', window.location.origin)
	}
	return url.toString()
})
</script>

<template>
	<div class="absolute inset-x-6 bottom-[5.75rem] top-[4.75rem] flex items-center justify-center">
		<iframe
			v-if="embedUrl"
			:src="embedUrl"
			:title="item.title || item.alt"
			class="youtube-player"
			allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
			referrerpolicy="strict-origin-when-cross-origin"
			allowfullscreen
		/>
	</div>
	<div
		class="absolute bottom-6 left-1/2 z-10 flex max-w-[calc(100%_-_3rem)] -translate-x-1/2 items-center gap-2 rounded-[20px] border border-solid border-white/10 bg-surface-3 p-2"
		@click.stop
	>
		<template v-if="count > 1">
			<IconButton
				:label="formatMessage(commonMessages.backButton)"
				type="quiet"
				@click="emit('previous')"
			>
				<LeftArrowIcon aria-hidden="true" />
			</IconButton>
			<span
				class="flex min-w-14 justify-center gap-1 font-semibold tabular-nums text-secondary"
				aria-live="polite"
			>
				<strong class="text-contrast">{{ index + 1 }}</strong
				><span>/ {{ count }}</span>
			</span>
			<IconButton
				:label="formatMessage(commonMessages.nextButton)"
				type="quiet"
				@click="emit('next')"
			>
				<RightArrowIcon aria-hidden="true" />
			</IconButton>
			<div class="h-6 w-px bg-white/10" />
		</template>
		<slot name="actions" />
		<IconButton :label="formatMessage(messages.close)" type="quiet" @click="emit('close')">
			<XIcon aria-hidden="true" />
		</IconButton>
	</div>
</template>

<style scoped>
.youtube-player {
	width: min(100%, calc((100vh - 12rem) * 16 / 9));
	aspect-ratio: 16 / 9;
	border: 0;
}
</style>
