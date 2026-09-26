import type { StackedAdmonitionItem } from '@orbiont/ui'

export type InstanceAdmonitionKind = 'locked'

export type InstanceAdmonitionItem = StackedAdmonitionItem & {
	kind: InstanceAdmonitionKind
}
