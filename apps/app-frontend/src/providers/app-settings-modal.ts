import type { InjectionKey } from 'vue'

export type UnsavedChangesController = {
	hasChanges: () => boolean
	getOriginal: () => Record<string, unknown>
	getModified: () => Record<string, unknown>
	isSaving: () => boolean
	canSave?: () => boolean
	reset: () => void
	save: () => void | Promise<void>
}

export type AppSettingsModalContext = {
	close: () => boolean
	registerUnsavedChangesController: (
		controller: UnsavedChangesController | null,
		key?: string,
	) => void
}

export const appSettingsModalContextKey: InjectionKey<AppSettingsModalContext> =
	Symbol('appSettingsModalContext')
export const appSettingsModalOpenProfileKey: InjectionKey<() => void> = Symbol(
	'appSettingsModalOpenProfile',
)
export const appSettingsModalOpenSyncedOptionsKey: InjectionKey<() => void> = Symbol(
	'appSettingsModalOpenSyncedOptions',
)

// Nested settings sections share one action bar without replacing each other's drafts.
export function combineSettingsChanges(
	controllers: Map<string, UnsavedChangesController>,
): UnsavedChangesController {
	return {
		hasChanges: () => [...controllers.values()].some((controller) => controller.hasChanges()),
		isSaving: () => [...controllers.values()].some((controller) => controller.isSaving()),
		canSave: () => [...controllers.values()].every((controller) => controller.canSave?.() ?? true),
		getOriginal: () =>
			Object.fromEntries(
				[...controllers].map(([key, controller]) => [key, controller.getOriginal()]),
			),
		getModified: () =>
			Object.fromEntries(
				[...controllers]
					.filter(([, controller]) => controller.hasChanges())
					.map(([key, controller]) => [
						key,
						{ ...controller.getOriginal(), ...controller.getModified() },
					]),
			),
		reset: () => {
			for (const controller of controllers.values()) controller.reset()
		},
		save: async () => {
			if ([...controllers.values()].some((controller) => controller.isSaving())) return
			if (![...controllers.values()].every((controller) => controller.canSave?.() ?? true)) return
			// Serial saves preserve settings changed by other sections during read/merge/write.
			for (const controller of controllers.values()) {
				if (controller.hasChanges()) await controller.save()
			}
		},
	}
}
