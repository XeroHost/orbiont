<template>
	<StackedAdmonitions v-bind="$attrs" :items="stackItems" class="w-full">
		<template #item="{ item }">
			<InstanceAdmonitionsLocked v-if="item.kind === 'locked'" @delete="emit('delete')" />
		</template>
	</StackedAdmonitions>
</template>

<script setup lang="ts">
import { StackedAdmonitions } from '@modrinth/ui'
import { computed } from 'vue'

import type { GameInstance } from '@/helpers/types'

import InstanceAdmonitionsLocked from './locked.vue'
import type { InstanceAdmonitionItem } from './types.ts'

defineOptions({
	inheritAttrs: false,
})

const props = defineProps<{
	instance: GameInstance
}>()

const emit = defineEmits<{
	delete: []
}>()

const stackItems = computed<InstanceAdmonitionItem[]>(() =>
	props.instance.quarantined
		? [{ id: 'locked', type: 'warning', dismissible: false, kind: 'locked' }]
		: [],
)
</script>
