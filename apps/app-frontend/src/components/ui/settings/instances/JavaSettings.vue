<script setup>
import { defineMessages, useSavable, useVIntl } from '@orbiont/ui'
import { ref } from 'vue'

import JavaSelector from '@/components/ui/JavaSelector.vue'
import { useSettingsChanges } from '@/composables/use-settings-changes'
import { get_java_versions, set_java_version } from '@/helpers/jre'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	javaLocation: {
		id: 'app.settings.java-installations.location.title',
		defaultMessage: 'Java {version, number} location',
	},
})

const persistedVersions = ref(await get_java_versions())
const draft = useSavable(
	() => persistedVersions.value,
	async (changes) => {
		for (const [major, version] of Object.entries(changes)) {
			await set_java_version(version)
			persistedVersions.value = { ...persistedVersions.value, [major]: { ...version } }
		}
	},
)
const javaVersions = draft.current
useSettingsChanges('java-installations', {
	hasChanges: () => draft.hasChanges.value,
	getOriginal: () => draft.saved.value,
	getModified: () => draft.changes.value,
	isSaving: () => draft.saving.value,
	reset: draft.reset,
	save: draft.save,
})

function updateJavaVersion(version) {
	if (version?.path === '') {
		version.path = undefined
	}

	if (version?.path) {
		version.path = version.path.replace('java.exe', 'javaw.exe')
	}
}
</script>
<template>
	<div class="flex flex-col gap-6">
		<div
			v-for="(javaVersion, index) in [25, 21, 17, 8]"
			:key="`java-${javaVersion}`"
			class="flex flex-col gap-2.5"
		>
			<h2 class="m-0 text-lg font-semibold text-contrast" :class="{ 'mt-4': index !== 0 }">
				{{ formatMessage(messages.javaLocation, { version: javaVersion }) }}
			</h2>
			<JavaSelector
				:id="'java-selector-' + javaVersion"
				v-model="javaVersions[javaVersion]"
				:version="javaVersion"
				@update:model-value="updateJavaVersion"
			/>
		</div>
	</div>
</template>
