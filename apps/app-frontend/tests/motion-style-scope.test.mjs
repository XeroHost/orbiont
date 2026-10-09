import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import postcss from 'postcss'
import * as sass from 'sass'
import { compileStyle, parse } from 'vue/compiler-sfc'

const files = [
	'apps/app-frontend/src/App.vue',
	'apps/app-frontend/src/components/ui/QuickInstanceSwitcher.vue',
	'apps/app-frontend/src/components/ui/world/RecentWorldsList.vue',
	'apps/app-frontend/src/components/ui/sidebar/SidebarHostCard.vue',
	'apps/app-frontend/src/components/ui/download-manager/download-manager-bar.vue',
	'packages/ui/src/components/base/Accordion.vue',
	'packages/ui/src/components/base/Collapsible.vue',
	'packages/ui/src/components/base/FloatingActionBar.vue',
	'packages/ui/src/components/base/Slider.vue',
	'packages/ui/src/components/search/SearchFilterGroup.vue',
	'packages/ui/src/components/skin/SkinButton.vue',
	'packages/ui/src/components/modal/NewModal.vue',
]

test('compiled motion styles target components, never the document itself', () => {
	for (const file of files) {
		const source = readFileSync(new URL(`../../../${file}`, import.meta.url), 'utf8')
		const { descriptor } = parse(source, { filename: file })
		let motionRules = 0
		for (const style of descriptor.styles.filter((style) => style.scoped)) {
			const css = style.lang === 'scss' ? sass.compileString(style.content).css : style.content
			const compiled = compileStyle({
				source: css,
				filename: file,
				id: 'data-v-test',
				scoped: true,
			})
			assert.deepEqual(compiled.errors, [], file)
			postcss.parse(compiled.code).walkRules((rule) => {
				if (!rule.selector.includes('data-reduced-motion')) return
				motionRules++
				assert.doesNotMatch(
					rule.selector,
					/^html(?:\[data-reduced-motion[^\]]+\]|:not\(\[data-reduced-motion[^\]]+\]\))$/,
					`${file}: ${rule.selector} would change visibility or animation of the entire launcher`,
				)
				assert.match(rule.selector, /\[data-v-test\]/, `${file}: component scope was lost`)
			})
		}
		assert.ok(motionRules > 0, `${file}: motion rules were not tested`)
	}
})
