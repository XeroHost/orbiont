<template>
	<div>
		<h2 class="m-0 mb-4 text-xl font-semibold text-contrast">
			{{ formatMessage(commonSettingsMessages.language) }}
		</h2>

		<LanguageSettingsSelector
			:product="product"
			:current-locale="current.locale"
			:locales="LOCALES"
			:on-locale-change="onLocaleChange"
			:is-changing="saving"
			:coverage-by-locale="languageCoverage[product]"
		/>
	</div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'

import { LOCALES, useVIntl } from '#ui/composables'
import { injectI18n } from '#ui/providers'
import { commonSettingsMessages, useSavable } from '#ui/utils'

import { languageCoverage } from './language-settings-coverage.generated'
import LanguageSettingsSelector from './language-settings-selector.vue'

const props = defineProps<{
	product: 'app'
	persistLocale?: (locale: string) => void | Promise<void>
}>()

const { formatMessage } = useVIntl()
const { locale, setLocale } = injectI18n()
const persistedLocale = ref(locale.value)
let localeChangeQueue = Promise.resolve()

const { saved, current, changes, saving, hasChanges, reset, save } = useSavable(
	() => ({ locale: persistedLocale.value }),
	async () => {
		await queueLocaleChange(current.value.locale, true)
		await props.persistLocale?.(current.value.locale)
		persistedLocale.value = current.value.locale
	},
)

function queueLocaleChange(newLocale: string, persist = false): Promise<void> {
	const request = localeChangeQueue.then(() => Promise.resolve(setLocale(newLocale, { persist })))
	localeChangeQueue = request.catch(() => undefined)
	return request
}

function onLocaleChange(newLocale: string): void {
	current.value.locale = newLocale
	void queueLocaleChange(newLocale).catch(() => undefined)
}

function resetLanguageSettings(): void {
	reset()
	void queueLocaleChange(current.value.locale).catch(() => undefined)
}

async function saveLanguageSettings(): Promise<void> {
	try {
		await save()
	} catch {
		return
	}
}

onBeforeUnmount(() => {
	if (hasChanges.value || locale.value !== persistedLocale.value) {
		void queueLocaleChange(persistedLocale.value).catch(() => undefined)
	}
})

defineExpose({
	originalState: saved,
	modifiedState: changes,
	hasChanges,
	saving,
	reset: resetLanguageSettings,
	save: saveLanguageSettings,
})
</script>
