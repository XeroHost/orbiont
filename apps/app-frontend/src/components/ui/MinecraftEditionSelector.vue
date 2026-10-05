<script setup lang="ts">
import { CheckIcon } from '@orbiont/assets'
import { Combobox, useVIntl } from '@orbiont/ui'
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import bedrockLogo from '@/assets/editions/bedrock-edition.png?url'
import javaLogo from '@/assets/editions/java-edition.png?url'
import { bedrockMessages } from '@/helpers/bedrock-messages'

withDefaults(defineProps<{ expanded?: boolean }>(), { expanded: true })

const { formatMessage } = useVIntl()
const route = useRoute()
const router = useRouter()
const edition = computed(() =>
	route.path.startsWith('/bedrock') || route.query.edition === 'bedrock' ? 'bedrock' : 'java',
)
const options = computed(() => [
	{ value: 'java', label: formatMessage(bedrockMessages.java), logo: javaLogo },
	{ value: 'bedrock', label: formatMessage(bedrockMessages.bedrock), logo: bedrockLogo },
])
function changeEdition(value: string) {
	if (value === edition.value) return
	void router.push(value === 'bedrock' ? '/bedrock' : '/')
}
</script>

<template>
	<div
		v-tooltip.right="
			expanded ? undefined : options.find((option) => option.value === edition)?.label
		"
		role="group"
		:aria-label="formatMessage(bedrockMessages.edition)"
		class="w-full"
	>
		<Combobox
			:model-value="edition"
			:options="options"
			:show-chevron="expanded"
			:dropdown-min-width="224"
			trigger-class="!h-12 !px-3 !text-sm !rounded-xl"
			trigger-type="outlined"
			@update:model-value="changeEdition"
		>
			<template #prefix>
				<img
					:src="edition === 'bedrock' ? bedrockLogo : javaLogo"
					alt=""
					class="size-6 shrink-0 object-contain"
					:class="{ 'rounded-md [image-rendering:pixelated]': edition === 'bedrock' }"
				/>
			</template>
			<template #selected="{ label }">
				<span :class="{ 'sr-only': !expanded }">{{ label }}</span>
			</template>
			<template #option="{ item }">
				<img
					:src="item.value === 'bedrock' ? bedrockLogo : javaLogo"
					alt=""
					class="size-7 shrink-0 object-contain"
					:class="{ 'rounded-md [image-rendering:pixelated]': item.value === 'bedrock' }"
				/>
				<span class="min-w-0 flex-1 font-semibold">{{ item.label }}</span>
				<CheckIcon v-if="item.value === edition" class="size-4 shrink-0 text-contrast" />
			</template>
		</Combobox>
	</div>
</template>
