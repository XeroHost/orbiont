<script setup lang="ts">
import { ChevronLeftIcon, ChevronRightIcon, CubeIcon, PauseIcon, PlayIcon } from '@orbiont/assets'
import {
	Button,
	commonMessages,
	defineMessages,
	IconButton,
	useFormatNumber,
	useVIntl,
} from '@orbiont/ui'
import { useDocumentVisibility } from '@vueuse/core'
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'

import { useFeaturedModpacks } from '@/composables/use-featured-modpacks'

const { formatMessage } = useVIntl()
const formatNumber = useFormatNumber()
const { items, loading, retry } = useFeaturedModpacks()
const messages = defineMessages({
	title: { id: 'app.sidebar.featured.title', defaultMessage: 'Featured modpacks' },
	view: { id: 'app.sidebar.featured.view', defaultMessage: 'View project' },
	previous: { id: 'app.sidebar.featured.previous', defaultMessage: 'Previous modpack' },
	next: { id: 'app.sidebar.featured.next', defaultMessage: 'Next modpack' },
	position: { id: 'app.sidebar.featured.position', defaultMessage: '{position} of {total}' },
	pause: { id: 'app.sidebar.featured.pause', defaultMessage: 'Pause rotation' },
	resume: { id: 'app.sidebar.featured.resume', defaultMessage: 'Resume rotation' },
	unavailable: {
		id: 'app.sidebar.featured.unavailable',
		defaultMessage: 'Featured modpacks are unavailable right now.',
	},
})
const providers = { modrinth: 'Modrinth', curseforge: 'CurseForge' }
const selectedId = ref<string | null>(null)
const currentIndex = computed(() =>
	Math.max(
		0,
		items.value.findIndex((item) => item.projectId === selectedId.value),
	),
)
const current = computed(() => items.value[currentIndex.value])
const paused = ref(false)
const hovered = ref(false)
const focused = ref(false)
const visibility = useDocumentVisibility()
const failedImages = ref<string[]>([])
const displayedImage = computed(() =>
	[current.value?.imageUrl, current.value?.iconUrl].find(
		(url) => !!url && !failedImages.value.includes(url),
	),
)
let timer: ReturnType<typeof setTimeout> | undefined

watch(
	items,
	(next) => {
		if (!next.some((item) => item.projectId === selectedId.value))
			selectedId.value = next[0]?.projectId ?? null
	},
	{ immediate: true },
)
watch(
	() => current.value?.projectId,
	() => {
		failedImages.value = []
	},
)

function advance(direction: number) {
	if (items.value.length < 2) return
	const index = (currentIndex.value + direction + items.value.length) % items.value.length
	selectedId.value = items.value[index].projectId
}

function leaveFocus(event: FocusEvent) {
	focused.value =
		!!event.relatedTarget &&
		(event.currentTarget as HTMLElement).contains(event.relatedTarget as Node)
}

watch(
	[items, selectedId, paused, hovered, focused, visibility],
	() => {
		clearTimeout(timer)
		if (
			items.value.length > 1 &&
			!paused.value &&
			!hovered.value &&
			!focused.value &&
			visibility.value === 'visible'
		) {
			timer = setTimeout(() => advance(1), 8000)
		}
	},
	{ immediate: true },
)
onBeforeUnmount(() => clearTimeout(timer))
</script>

<template>
	<section
		class="featured-modpacks"
		:aria-label="formatMessage(messages.title)"
		@mouseenter="hovered = true"
		@mouseleave="hovered = false"
		@focusin="focused = true"
		@focusout="leaveFocus"
	>
		<h2>{{ formatMessage(messages.title) }}</h2>
		<div :aria-live="paused || hovered || focused ? 'polite' : 'off'">
			<RouterLink
				v-if="current"
				:to="`/project/${encodeURIComponent(current.projectId)}`"
				class="featured-project no-click-animation"
			>
				<div class="featured-cover">
					<img
						v-if="displayedImage"
						:key="displayedImage"
						:src="displayedImage"
						alt=""
						:class="{ 'icon-cover': displayedImage === current.iconUrl }"
						@error="failedImages.push(displayedImage!)"
					/>
					<CubeIcon v-else class="featured-fallback" aria-hidden="true" />
					<span class="featured-provider">{{ providers[current.provider] }}</span>
				</div>
				<div class="featured-details">
					<h3>{{ current.name }}</h3>
					<p>{{ current.summary }}</p>
					<span class="featured-action"
						>{{ formatMessage(messages.view) }}<ChevronRightIcon aria-hidden="true"
					/></span>
				</div>
			</RouterLink>
			<div v-else class="featured-empty" role="status">
				<p>{{ formatMessage(loading ? commonMessages.loadingLabel : messages.unavailable) }}</p>
				<Button v-if="!loading" size="sm" @click="retry">{{
					formatMessage(commonMessages.retryButton)
				}}</Button>
			</div>
		</div>
		<div v-if="items.length > 1" class="featured-controls">
			<IconButton size="sm" :label="formatMessage(messages.previous)" @click="advance(-1)"
				><ChevronLeftIcon
			/></IconButton>
			<span
				class="featured-position"
				:aria-label="
					formatMessage(messages.position, { position: currentIndex + 1, total: items.length })
				"
				>{{ formatNumber(currentIndex + 1) }} / {{ formatNumber(items.length) }}</span
			>
			<IconButton
				size="sm"
				:label="formatMessage(paused ? messages.resume : messages.pause)"
				:aria-pressed="paused"
				@click="paused = !paused"
				><PlayIcon v-if="paused" /><PauseIcon v-else
			/></IconButton>
			<IconButton size="sm" :label="formatMessage(messages.next)" @click="advance(1)"
				><ChevronRightIcon
			/></IconButton>
		</div>
	</section>
</template>

<style scoped lang="scss">
.featured-modpacks {
	margin: 12px;
	flex-shrink: 0;
	min-width: 0;
	h2 {
		margin: 0 0 10px;
		font-size: 14px;
		font-weight: 700;
		color: var(--color-contrast);
	}
}
.featured-project {
	display: block;
	overflow: hidden;
	border: 1px solid var(--surface-5);
	border-radius: 12px;
	background: var(--surface-4);
	color: var(--color-primary);
	text-decoration: none;
	transform: none;
	&:active {
		transform: none;
	}
	&:focus-visible {
		outline: 2px solid var(--color-brand);
		outline-offset: 2px;
	}
	&:hover .featured-action {
		color: var(--color-brand);
	}
}
.featured-cover {
	position: relative;
	height: 80px;
	display: flex;
	align-items: center;
	justify-content: center;
	background: var(--surface-3);
	img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
	img.icon-cover {
		object-fit: contain;
		padding: 8px;
		box-sizing: border-box;
	}
}
.featured-fallback {
	width: 40px;
	height: 40px;
	color: var(--color-secondary);
}
.featured-provider {
	position: absolute;
	bottom: 6px;
	left: 8px;
	padding: 3px 6px;
	border-radius: 6px;
	background: var(--surface-2);
	color: var(--color-contrast);
	font-size: 10px;
	font-weight: 600;
}
.featured-details {
	padding: 10px;
	h3,
	p {
		display: -webkit-box;
		-webkit-box-orient: vertical;
		-webkit-line-clamp: 2;
		overflow: hidden;
		overflow-wrap: anywhere;
	}
	h3 {
		margin: 0;
		font-size: 15px;
		line-height: 1.3;
		min-height: 2.6em;
		color: var(--color-contrast);
	}
	p {
		margin: 6px 0 8px;
		font-size: 12px;
		line-height: 1.4;
		min-height: 2.8em;
		color: var(--color-secondary);
	}
}
.featured-action {
	display: flex;
	align-items: center;
	justify-content: space-between;
	gap: 8px;
	font-size: 12px;
	font-weight: 600;
	svg {
		width: 16px;
		height: 16px;
		flex-shrink: 0;
	}
}
.featured-controls {
	display: flex;
	align-items: center;
	gap: 4px;
	margin-top: 8px;
}
.featured-position {
	flex: 1;
	text-align: center;
	font-size: 12px;
	color: var(--color-secondary);
	font-variant-numeric: tabular-nums;
}
.featured-empty {
	display: flex;
	flex-direction: column;
	align-items: center;
	gap: 8px;
	padding: 16px;
	font-size: 12px;
	color: var(--color-secondary);
	text-align: center;
}
</style>
