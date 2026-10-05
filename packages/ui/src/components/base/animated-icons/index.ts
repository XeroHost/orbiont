import { defineComponent, h, markRaw } from 'vue'

import AnimatedIcon from './AnimatedIcon.vue'
import type { AnimatedIconName } from './registry'

export { AnimatedIcon }
export type { AnimatedIconName } from './registry'

export function animatedIcon(name: AnimatedIconName) {
	return markRaw(
		defineComponent({
			name: `HugeAnimated-${name}`,
			inheritAttrs: false,
			setup:
				(_, { attrs }) =>
				() =>
					h(AnimatedIcon, { ...attrs, name }),
		}),
	)
}
