import type { CatalogMessages } from './composables/i18n'

export const uiLocaleModulesEager = import.meta.glob<{ default: CatalogMessages }>(
	'./locales/*/index.json',
	{ eager: true },
)
