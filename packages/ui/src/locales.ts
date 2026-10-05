import type { CatalogMessages } from './composables/i18n'

export const uiLocaleModules = import.meta.glob<{ default: CatalogMessages }>(
	'./locales/*/index.json',
	{ eager: false },
)

export const metaLocaleModules = import.meta.glob<{ default: CatalogMessages }>(
	'./locales/*/meta.json',
	{ eager: true },
)
