<script setup lang="ts">
import SidebarAdSlots from './SidebarAdSlots.vue'
import SidebarFeaturedModpacks from './SidebarFeaturedModpacks.vue'
import SidebarHostCard from './SidebarHostCard.vue'

defineProps<{ layout: 'full' | 'catalog' | 'none'; bedrock?: boolean }>()
defineEmits<{ visitHosting: [] }>()

const adPreview = import.meta.env.DEV
</script>

<template>
	<SidebarHostCard v-if="layout === 'full'" @visit="$emit('visitHosting')" />
	<SidebarFeaturedModpacks v-if="layout === 'full' && !bedrock" />
	<div
		id="sidebar-teleport-target"
		class="sidebar-teleport-content"
		:class="{ 'with-ad-preview': layout === 'catalog' && adPreview }"
	></div>
	<SidebarAdSlots v-if="layout !== 'none'" :count="layout === 'catalog' ? 1 : 2" />
</template>

<style scoped lang="scss">
.sidebar-teleport-content {
	display: contents;
}

.sidebar-teleport-content.with-ad-preview:not(:empty) {
	display: block;
	flex: 1;
	min-height: 12rem;
	max-height: max(12rem, calc(100vh - var(--top-bar-height, 48px) - 250px));
	overflow-y: auto;
	overflow-x: hidden;
	scrollbar-gutter: stable;
}

.sidebar-teleport-content:empty {
	display: none;
}
</style>
