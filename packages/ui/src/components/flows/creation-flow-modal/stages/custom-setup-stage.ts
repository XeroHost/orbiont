import { LeftArrowIcon, PlusIcon } from '@orbiont/assets'
import { markRaw } from 'vue'

import { commonMessages } from '#ui/utils/common-messages'

import type { StageConfigInput } from '../../../base'
import CustomSetupStage from '../components/CustomSetupStage.vue'
import { type CreationFlowContextValue, creationFlowMessages } from '../creation-flow-context'

function isForwardBlocked(ctx: CreationFlowContextValue): boolean {
	if (ctx.optifineBusy.value || (ctx.optifineEnabled.value && !ctx.optifineInstaller.value))
		return true
	if (!ctx.selectedGameVersion.value) return true
	if (!ctx.hideLoaderChips.value && !ctx.selectedLoader.value) return true
	if (!ctx.hideLoaderVersion.value && !ctx.selectedLoaderVersion.value) return true
	return false
}

export const stageConfig: StageConfigInput<CreationFlowContextValue> = {
	id: 'custom-setup',
	title: (ctx) => ctx.formatMessage(creationFlowMessages.createInstanceTitle),
	stageContent: markRaw(CustomSetupStage),
	skip: (ctx) =>
		ctx.setupType.value === 'modpack' ||
		ctx.setupType.value === 'vanilla' ||
		ctx.isImportMode.value,
	cannotNavigateForward: isForwardBlocked,
	leftButtonConfig: (ctx) => ({
		label: ctx.formatMessage(commonMessages.backButton),
		icon: LeftArrowIcon,
		onClick: () => ctx.modal.value?.setStage('setup-type'),
	}),
	rightButtonConfig: (ctx) => ({
		label: ctx.formatMessage(creationFlowMessages.createInstanceButton),
		icon: PlusIcon,
		iconPosition: 'before' as const,
		color: 'brand' as const,
		disabled: isForwardBlocked(ctx) || ctx.finishDisabled.value,
		loading: ctx.loading.value,
		tooltip: ctx.finishDisabled.value ? ctx.finishDisabledTooltip.value : undefined,
		onClick: () => ctx.finish(),
	}),
	maxWidth: '560px',
}
