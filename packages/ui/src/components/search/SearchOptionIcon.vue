<script setup lang="ts">
import { type Component, computed, toRaw } from 'vue'

import AnimatedIcon from '../base/animated-icons/AnimatedIcon.vue'
import { animatedCategories } from '../base/animated-icons/category-icons'

const props = defineProps<{ icon: string | Component }>()
const name = computed(() =>
	typeof props.icon === 'string' ? undefined : animatedCategories.get(toRaw(props.icon)),
)
</script>

<template>
	<AnimatedIcon v-if="name" :name="name" />
	<!-- Preserve SVG markup rendering for loaders and other non-category icons. -->
	<!-- eslint-disable-next-line vue/no-v-html -->
	<div v-else-if="typeof icon === 'string'" v-html="icon" />
	<component :is="icon" v-else />
</template>
