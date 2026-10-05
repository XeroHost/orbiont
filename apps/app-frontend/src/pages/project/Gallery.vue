<template>
	<div class="gallery">
		<Card v-for="(image, index) in galleryMedia" :key="image.id" class="gallery-item">
			<button type="button" class="gallery-preview" @click="expandImage(index)">
				<img
					:src="image.thumbnail"
					:alt="image.title || formatMessage(messages.galleryImageFallback)"
					class="gallery-image"
				/>
				<span v-if="image.youtubeVideoId" class="gallery-play" aria-hidden="true"
					><PlayIcon
				/></span>
			</button>
			<div class="gallery-body">
				<h3>{{ image.title }}</h3>
				{{ image.description }}
			</div>
			<span v-if="image.created" class="gallery-time">
				<CalendarIcon />
				{{ formatDate(new Date(image.created)) }}
			</span>
		</Card>
	</div>
	<ImageViewerEditor ref="galleryViewer" :items="galleryViewerItems" editor="disabled">
		<template #actions="{ item }">
			<Button
				type="quiet"
				class="!w-9 !rounded-full !p-0"
				:aria-label="formatMessage(commonMessages.openInBrowserButton)"
				@click="openUrl(item.src)"
			>
				<ExternalIcon aria-hidden="true" />
			</Button>
		</template>
	</ImageViewerEditor>
</template>

<script setup>
import { CalendarIcon, ExternalIcon, PlayIcon } from '@orbiont/assets'
import {
	Button,
	Card,
	commonMessages,
	defineMessages,
	ImageViewerEditor,
	useFormatDateTime,
	useVIntl,
} from '@orbiont/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, ref } from 'vue'

import { getProjectGalleryMedia } from '@/helpers/project-gallery'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	galleryImageFallback: {
		id: 'app.project.gallery.image-fallback',
		defaultMessage: 'Gallery image',
	},
})

const formatDate = useFormatDateTime({
	year: 'numeric',
	month: 'long',
	day: 'numeric',
})

const props = defineProps({
	project: {
		type: Object,
		default: () => ({}),
	},
})

const galleryMedia = computed(() => getProjectGalleryMedia(props.project))

const galleryViewer = ref()
const galleryViewerItems = computed(() =>
	galleryMedia.value.map((image) => ({
		id: image.id,
		src: image.src,
		youtubeVideoId: image.youtubeVideoId,
		alt: image.title || formatMessage(messages.galleryImageFallback),
		title: image.title,
		description: image.description,
	})),
)

const expandImage = (index) => {
	galleryViewer.value?.show(index)
}
</script>

<style scoped lang="scss">
.gallery {
	display: grid;
	grid-template-columns: repeat(auto-fill, minmax(20rem, 1fr));
	width: 100%;
	gap: 1rem;
}

.gallery-item {
	padding: 0;
	overflow: hidden;
	margin: 0;
	display: flex;
	flex-direction: column;

	.gallery-preview {
		position: relative;
		padding: 0;
		border: 0;
		background: transparent;
		cursor: pointer;
	}

	.gallery-play {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		background: rgb(0 0 0 / 15%);

		svg {
			width: 3rem;
			height: 3rem;
			padding: 0.75rem;
			border-radius: 50%;
			color: white;
			background: rgb(0 0 0 / 70%);
		}
	}

	.gallery-image {
		width: 100%;
		aspect-ratio: 2/1;
		object-fit: cover;
		object-position: center;
	}

	.gallery-body {
		flex-grow: 1;
		padding: 1rem;
	}

	.gallery-time {
		padding: 0 1rem 1rem;
		vertical-align: center;
	}
}
</style>
