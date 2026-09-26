import type { StackedAdmonitionItem } from '@modrinth/ui'

export type InstanceAdmonitionKind = 'locked'

export type InstanceAdmonitionItem = StackedAdmonitionItem & {
	kind: InstanceAdmonitionKind
}
