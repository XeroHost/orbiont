<script setup lang="ts">
import { productName } from '@orbiont/branding'

import Wordmark from '@/assets/branding/orbiont-wordmark-adaptive.svg?component'
import { useAccent } from '@/composables/use-accent'

useAccent()
</script>

<template>
	<Wordmark :aria-label="productName" role="img" class="h-7 w-auto" />
</template>
