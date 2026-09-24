<script setup lang="ts">
import { productName } from '@modrinth/branding'
import { computed } from 'vue'

import wordmarkDark from '@/assets/branding/orbiont-wordmark-cyan.svg?url'
import wordmarkLight from '@/assets/branding/orbiont-wordmark-dark.svg?url'
import { isDarkTheme, useTheme } from '@/composables/use-theme.ts'

const theme = useTheme()
const wordmarkSrc = computed(() => (isDarkTheme(theme.active) ? wordmarkDark : wordmarkLight))
</script>

<template>
	<img :src="wordmarkSrc" :alt="productName" class="h-7 w-auto" />
</template>
