import type { Ref } from 'vue'
import { computed, ref, watch } from 'vue'

export interface ScrollViewportOptions {
	onScroll?: () => void
	onResize?: () => void
}

export interface VirtualScrollOptions {
	itemHeight: number
	bufferSize?: number
	initialItemCount?: number
	enabled?: Ref<boolean>
	onNearEnd?: () => void
	nearEndThreshold?: number
}

export function findScrollableAncestor(element: HTMLElement | null): HTMLElement | Window {
	if (!element) return window

	let current: HTMLElement | null = element.parentElement
	while (current) {
		const { overflowY } = getComputedStyle(current)
		if (overflowY === 'auto' || overflowY === 'scroll') {
			return current
		}
		current = current.parentElement
	}
	return window
}

export function getScrollTop(container: HTMLElement | Window): number {
	return container instanceof Window ? window.scrollY : container.scrollTop
}

export function getViewportHeight(container: HTMLElement | Window): number {
	return container instanceof Window ? window.innerHeight : container.clientHeight
}

export function useScrollViewport(options: ScrollViewportOptions = {}) {
	const listContainer = ref<HTMLElement | null>(null)
	const scrollContainer = ref<HTMLElement | Window | null>(null)
	const scrollTop = ref(0)
	const viewportHeight = ref(0)
	const containerOffset = ref(0)
	const relativeScrollTop = computed(() => Math.max(0, scrollTop.value - containerOffset.value))
	let frame: number | null = null
	let scrollPending = false
	let resizePending = false
	let active = false

	function updateContainerOffset() {
		const listEl = listContainer.value
		const container = scrollContainer.value
		if (!listEl || !container) return
		const listTop = listEl.getBoundingClientRect().top
		containerOffset.value =
			container instanceof Window
				? listTop + window.scrollY
				: listTop -
					container.getBoundingClientRect().top -
					container.clientTop +
					container.scrollTop
	}

	function syncScrollState() {
		const listEl = listContainer.value
		if (!listEl || typeof window === 'undefined') return
		const container = findScrollableAncestor(listEl)
		scrollContainer.value = container
		scrollTop.value = getScrollTop(container)
		viewportHeight.value = getViewportHeight(container)
		updateContainerOffset()
	}

	function resetScrollState() {
		scrollTop.value = 0
		viewportHeight.value = 0
		containerOffset.value = 0
	}

	function scheduleUpdate(kind: 'scroll' | 'resize') {
		if (!active) return
		if (kind === 'scroll') scrollPending = true
		else resizePending = true
		frame ??= window.requestAnimationFrame(() => {
			frame = null
			const didScroll = scrollPending
			const didResize = resizePending
			scrollPending = resizePending = false
			syncScrollState()
			if (didScroll) options.onScroll?.()
			if (didResize) options.onResize?.()
		})
	}

	const handleScroll = () => scheduleUpdate('scroll')
	const handleResize = () => scheduleUpdate('resize')

	// Explicit watches avoid tracking geometry reads and repeatedly rebinding on scroll.
	watch(
		scrollContainer,
		(container, _, onCleanup) => {
			if (!container) return
			container.addEventListener('scroll', handleScroll, { passive: true })
			onCleanup(() => container.removeEventListener('scroll', handleScroll))
		},
		{ flush: 'sync' },
	)

	watch(
		listContainer,
		(listEl, _, onCleanup) => {
			if (!listEl || typeof window === 'undefined') return
			active = true
			syncScrollState()
			window.addEventListener('resize', handleResize, { passive: true })

			// Siblings can shift a list without changing its own size (headers, banners).
			let attached = true
			let layoutFrame: number | null = null
			let observedElements = new Set<Element>()
			const observer =
				typeof ResizeObserver === 'undefined'
					? undefined
					: new ResizeObserver(() => {
							if (attached) handleResize()
						})
			const layoutObserver =
				typeof MutationObserver === 'undefined'
					? undefined
					: new MutationObserver((mutations) => {
							if (!attached) return
							if (mutations.some((mutation) => mutation.type === 'childList')) {
								layoutFrame ??= window.requestAnimationFrame(() => {
									layoutFrame = null
									observeLayout()
								})
							}
							handleResize()
						})
			const observedList = listEl
			function observeLayout() {
				const elements = new Set<Element>([observedList])
				let ancestor = observedList.parentElement
				while (ancestor) {
					elements.add(ancestor)
					for (const child of ancestor.children) elements.add(child)
					ancestor = ancestor.parentElement
				}
				if (
					elements.size === observedElements.size &&
					[...elements].every((element) => observedElements.has(element))
				)
					return
				for (const element of observedElements) {
					if (!elements.has(element)) observer?.unobserve(element)
				}
				layoutObserver?.disconnect()
				for (const element of elements) {
					if (!observedElements.has(element)) observer?.observe(element)
					layoutObserver?.observe(element, {
						childList: element !== observedList,
						attributes: true,
						attributeFilter: ['style', 'class', 'hidden'],
					})
				}
				observedElements = elements
			}
			observeLayout()

			onCleanup(() => {
				attached = false
				active = false
				if (frame !== null) window.cancelAnimationFrame(frame)
				if (layoutFrame !== null) window.cancelAnimationFrame(layoutFrame)
				frame = null
				scrollPending = resizePending = false
				window.removeEventListener('resize', handleResize)
				observer?.disconnect()
				layoutObserver?.disconnect()
				scrollContainer.value = null
			})
		},
		{ flush: 'post' },
	)

	return {
		resetScrollState,
		containerOffset,
		listContainer,
		relativeScrollTop,
		scrollContainer,
		scrollTop,
		syncScrollState,
		updateContainerOffset,
		viewportHeight,
	}
}
export function useVirtualScroll<T>(
	items: Readonly<Ref<readonly T[]>>,
	options: VirtualScrollOptions,
) {
	const {
		itemHeight,
		bufferSize = 5,
		initialItemCount = 20,
		enabled,
		onNearEnd,
		nearEndThreshold = 0.2,
	} = options

	const {
		containerOffset,
		listContainer,
		relativeScrollTop,
		resetScrollState,
		scrollContainer,
		scrollTop,
		syncScrollState,
		viewportHeight,
	} = useScrollViewport({
		onScroll: checkNearEnd,
	})

	const totalHeight = computed(() => items.value.length * itemHeight)

	const visibleRange = computed(() => {
		if (enabled && !enabled.value) {
			return { start: 0, end: items.value.length }
		}

		if (!listContainer.value || !scrollContainer.value) {
			return { start: 0, end: Math.min(items.value.length, initialItemCount) }
		}

		const start = Math.floor(relativeScrollTop.value / itemHeight)
		const visibleCount = Math.ceil(viewportHeight.value / itemHeight)
		const rangeSize = visibleCount + bufferSize * 2

		const rangeStart = Math.min(
			Math.max(0, start - bufferSize),
			Math.max(0, items.value.length - rangeSize),
		)
		const rangeEnd = Math.min(items.value.length, rangeStart + rangeSize)

		return {
			start: rangeStart,
			end: rangeEnd,
		}
	})

	const visibleTop = computed(() =>
		enabled && !enabled.value ? 0 : visibleRange.value.start * itemHeight,
	)

	const visibleItems = computed(() =>
		items.value.slice(visibleRange.value.start, visibleRange.value.end),
	)

	function scrollToIndex(index: number) {
		if (index < 0 || index >= items.value.length) return
		syncScrollState()
		if (!listContainer.value || !scrollContainer.value) return
		const top = containerOffset.value + index * itemHeight - (viewportHeight.value - itemHeight) / 2
		scrollContainer.value.scrollTo({ top: Math.max(0, top), behavior: 'instant' })
		syncScrollState()
	}

	function checkNearEnd() {
		if (!onNearEnd || !listContainer.value || !viewportHeight.value) return

		const remainingScroll =
			containerOffset.value + totalHeight.value - scrollTop.value - viewportHeight.value

		if (remainingScroll < viewportHeight.value * nearEndThreshold) {
			onNearEnd()
		}
	}

	watch(items, () => {
		syncScrollState()
	})

	return {
		listContainer,
		totalHeight,
		visibleRange,
		visibleTop,
		visibleItems,
		scrollToIndex,
		resetScrollState,
		syncScrollState,
	}
}
