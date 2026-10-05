import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { test } from 'node:test'

import { compileScript, parse } from '@vue/compiler-sfc'
import * as vue from 'vue'

const require = createRequire(import.meta.url)
const ts = require('typescript')

function evaluate(source, imports) {
	const exports = {}
	const code = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
	}).outputText
	new Function('require', 'exports', code)((name) => {
		if (name in imports) return imports[name]
		throw new Error(`Unexpected import: ${name}`)
	}, exports)
	return exports
}

async function withViewer(run, items = []) {
	let context
	let nativeStatus = 200
	const calls = []
	const image = new Blob(['gallery-image'], { type: 'image/png' })
	const previousFetch = globalThis.fetch
	globalThis.fetch = async (src) => {
		calls.push(['browser', src])
		if (new URL(src).hostname === 'media.forgecdn.net') throw new TypeError('Failed to fetch')
		return new Response(image)
	}
	const provider = evaluate(
		readFileSync(new URL('../src/providers/setup/image-viewer-editor.ts', import.meta.url), 'utf8'),
		{
			'@orbiont/ui': { provideImageViewerEditor: (value) => (context = value) },
			'@tauri-apps/plugin-fs': { readFile: async () => new Uint8Array([1, 2, 3]) },
			'@tauri-apps/plugin-http': {
				fetch: async (src) => {
					calls.push(['native', src])
					return new Response(image, { status: nativeStatus, statusText: 'Service Unavailable' })
				},
			},
		},
	)
	provider.setupImageViewerEditorProvider()
	const { descriptor } = parse(
		readFileSync(
			new URL('../../../packages/ui/src/components/image-viewer-editor/index.vue', import.meta.url),
			'utf8',
		),
	)
	const component = evaluate(compileScript(descriptor, { id: 'gallery-images-test' }).content, {
		vue: { ...vue, onMounted: () => {}, onBeforeUnmount: () => {} },
		'#ui/providers/image-viewer-editor': { injectImageViewerEditor: () => context },
		'./editor.vue': {},
		'./youtube.vue': {},
	}).default
	const scope = vue.effectScope()
	try {
		const viewer = scope.run(() =>
			component.setup({ items, editor: 'enabled' }, { expose: () => {}, emit: () => {} }),
		)
		await run({ viewer, image, calls, setNativeStatus: (value) => (nativeStatus = value) })
	} finally {
		scope.stop()
		globalThis.fetch = previousFetch
	}
}

test('CurseForge gallery opens when the CDN forbids browser fetch', async () => {
	await withViewer(async ({ viewer, calls }) => {
		const src = 'https://media.forgecdn.net/attachments/123/456/gallery.png'
		const data = await viewer.loadItemData({ id: 'cf-gallery', src, alt: 'Gallery' })
		assert.equal(await data.source.text(), 'gallery-image')
		assert.deepEqual(calls, [['native', src]])
	})
})

test('gallery includes embedded YouTube videos alongside images', () => {
	const { descriptor } = parse(
		readFileSync(new URL('../src/pages/project/Gallery.vue', import.meta.url), 'utf8'),
	)
	const component = evaluate(compileScript(descriptor, { id: 'gallery-videos-test' }).content, {
		vue,
		'@orbiont/assets': {},
		'@orbiont/ui': {
			defineMessages: (value) => value,
			useVIntl: () => ({ formatMessage: (message) => message.defaultMessage }),
			useFormatDateTime: () => String,
		},
		'@tauri-apps/plugin-opener': {},
		get '@/helpers/project-gallery'() {
			return evaluate(
				readFileSync(new URL('../src/helpers/project-gallery.ts', import.meta.url), 'utf8'),
				{},
			)
		},
	}).default
	const scope = vue.effectScope()
	try {
		const gallery = scope.run(() =>
			component.setup(
				{
					project: {
						gallery: [{ url: 'https://media.forgecdn.net/attachments/image.png', title: 'Image' }],
						body: '<iframe src="https://www.youtube.com/embed/QIwFXTVn-5o"></iframe><iframe src="https://www.youtube.com/embed/-HqUkTtxtoU"></iframe>',
					},
				},
				{ expose: () => {} },
			),
		)
		assert.equal(gallery.galleryViewerItems.value.length, 3)
		assert.deepEqual(
			gallery.galleryViewerItems.value.slice(1).map((item) => item.youtubeVideoId),
			['QIwFXTVn-5o', '-HqUkTtxtoU'],
		)
	} finally {
		scope.stop()
	}
})

test('video navigation never fetches the YouTube page as an image', async () => {
	const previousImage = globalThis.Image
	globalThis.Image = class {
		async decode() {}
	}
	try {
		await withViewer(
			async ({ viewer, calls }) => {
				viewer.show(0)
				assert.equal(viewer.activeItem.value.youtubeVideoId, 'QIwFXTVn-5o')
				assert.equal(viewer.canEdit.value, false)
				viewer.next()
				assert.equal(viewer.activeItem.value.youtubeVideoId, '-HqUkTtxtoU')
				viewer.previous()
				assert.equal(viewer.activeIndex.value, 0)
				assert.deepEqual(calls, [])
				viewer.hide()
				assert.equal(viewer.activeItem.value, null)
			},
			[
				{
					id: 'video-1',
					src: 'https://www.youtube.com/watch?v=QIwFXTVn-5o',
					youtubeVideoId: 'QIwFXTVn-5o',
					alt: 'Trailer 1',
					editorSource: { id: 'video-1', path: 'ignored' },
				},
				{
					id: 'video-2',
					src: 'https://www.youtube.com/watch?v=-HqUkTtxtoU',
					youtubeVideoId: '-HqUkTtxtoU',
					alt: 'Trailer 2',
				},
			],
		)
	} finally {
		if (previousImage === undefined) delete globalThis.Image
		else globalThis.Image = previousImage
	}
})

test('Modrinth gallery retains its browser loader', async () => {
	await withViewer(async ({ viewer, calls }) => {
		const src = 'https://cdn.modrinth.com/data/project/images/gallery.png'
		const data = await viewer.loadItemData({ id: 'mr-gallery', src, alt: 'Gallery' })
		assert.equal(await data.source.text(), 'gallery-image')
		assert.deepEqual(calls, [['browser', src]])
	})
})

test('local screenshot editing still reads the source file', async () => {
	await withViewer(async ({ viewer, calls }) => {
		const data = await viewer.loadItemData({
			id: 'screenshot',
			src: 'asset://localhost/screenshot.png',
			alt: 'Screenshot',
			editorSource: { id: 'screenshot', path: 'C:/screenshots/original.png' },
		})
		assert.deepEqual(new Uint8Array(await data.source.arrayBuffer()), new Uint8Array([1, 2, 3]))
		assert.deepEqual(calls, [])
	})
})

test('failed remote image loads can be retried after the CDN recovers', async () => {
	await withViewer(async ({ viewer, setNativeStatus }) => {
		const item = {
			id: 'cf-retry',
			src: 'https://media.forgecdn.net/attachments/123/456/gallery.png',
			alt: 'Gallery',
		}
		setNativeStatus(503)
		await assert.rejects(viewer.loadItemData(item), /Could not load image: Service Unavailable/)
		setNativeStatus(200)
		assert.equal(await (await viewer.loadItemData(item)).source.text(), 'gallery-image')
	})
})
