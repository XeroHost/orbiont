import { categoryIconMap, ClientIcon, ServerIcon } from '@orbiont/assets'
import type { Component } from 'vue'

import { type AnimatedIconName, categoryHugeIcons } from './registry'

// Component identity also resolves provider aliases registered in @orbiont/assets.
export const animatedCategories = new Map<Component, AnimatedIconName>(
	Object.entries(categoryIconMap).map(([key, icon]) => [icon, categoryHugeIcons[key]]),
)
animatedCategories.set(ClientIcon, 'layout')
animatedCategories.set(ServerIcon, 'server')
