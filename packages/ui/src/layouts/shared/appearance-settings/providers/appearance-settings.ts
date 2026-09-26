import { createContext } from '#ui/providers/create-context'

import type {
	AppearanceRef,
	AppearanceSetter,
	AppearanceTheme,
	AppearanceThemeSelection,
	ProjectDisplayLocation,
	ProjectLayout,
	ProjectLayoutSetting,
	SidebarPreferences,
	WritableAppearanceSetting,
} from '../types'

interface AppearanceSetting<T> {
	value: AppearanceRef<T>
	update: AppearanceSetter<T>
	disabled?: AppearanceRef<boolean>
}

interface ThemeSettings {
	current: AppearanceRef<AppearanceThemeSelection>
	options: AppearanceRef<readonly AppearanceThemeSelection[]>
	system: AppearanceRef<AppearanceTheme>
	preferredDark: AppearanceRef<AppearanceTheme>
	update: AppearanceSetter<AppearanceThemeSelection>
}

interface ProjectLayoutSettings {
	value: AppearanceRef<readonly ProjectLayoutSetting[]>
	update: (type: ProjectDisplayLocation, layout: ProjectLayout) => Promise<void>
}

interface SidebarSettings {
	value: AppearanceRef<SidebarPreferences>
	update: (key: keyof SidebarPreferences, enabled: boolean) => Promise<void>
}

export interface AppearanceSettingsContext {
	theme: ThemeSettings
	advancedRendering: AppearanceSetting<boolean>
	nativeDecorations?: AppearanceSetting<boolean>
	projectLayouts?: ProjectLayoutSettings
	externalLinksNewTab?: AppearanceSetting<boolean>
	sidebarPreferences?: SidebarSettings
}

export interface AppearanceSettingsProviderOptions {
	deferPersistence?: boolean
	theme: {
		current: AppearanceRef<AppearanceThemeSelection>
		options: AppearanceRef<readonly AppearanceThemeSelection[]>
		system: AppearanceRef<AppearanceTheme>
		preferredDark: AppearanceRef<AppearanceTheme>
		set: AppearanceSetter<AppearanceThemeSelection>
	}
	advancedRendering: WritableAppearanceSetting<boolean>
	nativeDecorations?: WritableAppearanceSetting<boolean>
	projectLayouts?: {
		value: AppearanceRef<readonly ProjectLayoutSetting[]>
		set: (type: ProjectDisplayLocation, layout: ProjectLayout) => void | Promise<void>
	}
	externalLinksNewTab?: WritableAppearanceSetting<boolean>
	sidebarPreferences?: {
		value: AppearanceRef<SidebarPreferences>
		set: (key: keyof SidebarPreferences, enabled: boolean) => void | Promise<void>
	}
}

const [injectAppearanceSettings, provideAppearanceSettingsContext] =
	createContext<AppearanceSettingsContext>('AppearanceSettingsLayout', 'appearanceSettings')

export { injectAppearanceSettings }

function createSetting<T>(setting: WritableAppearanceSetting<T>): AppearanceSetting<T> {
	return {
		value: setting.value,
		update: setting.set,
	}
}

export function provideAppearanceSettings(
	options: AppearanceSettingsProviderOptions,
): AppearanceSettingsContext {
	const projectLayouts = options.projectLayouts
	const sidebarPreferences = options.sidebarPreferences

	const context: AppearanceSettingsContext = {
		theme: {
			current: options.theme.current,
			options: options.theme.options,
			system: options.theme.system,
			preferredDark: options.theme.preferredDark,
			update: async (theme) => {
				await options.theme.set(theme)
			},
		},
		advancedRendering: createSetting(options.advancedRendering),
		nativeDecorations: options.nativeDecorations
			? createSetting(options.nativeDecorations)
			: undefined,
		projectLayouts: projectLayouts
			? {
					value: projectLayouts.value,
					update: async (type, layout) => {
						await projectLayouts.set(type, layout)
					},
				}
			: undefined,
		externalLinksNewTab: options.externalLinksNewTab
			? createSetting(options.externalLinksNewTab)
			: undefined,
		sidebarPreferences: sidebarPreferences
			? {
					value: sidebarPreferences.value,
					update: async (key, enabled) => {
						await sidebarPreferences.set(key, enabled)
					},
				}
			: undefined,
	}

	return provideAppearanceSettingsContext(context)
}
