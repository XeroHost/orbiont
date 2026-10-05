import assert from 'node:assert/strict'
import { test } from 'node:test'

import { getProjectGalleryMedia, getYouTubeVideoId } from '../src/helpers/project-gallery.ts'

test('recognizes YouTube embeds, watch links and short URLs without retaining player parameters', () => {
	for (const url of [
		'https://www.youtube.com/embed/QIwFXTVn-5o?autoplay=1',
		'https://www.youtube.com/watch?feature=shared&amp;v=QIwFXTVn-5o',
		'https://youtu.be/QIwFXTVn-5o?t=30',
		'https://www.youtube-nocookie.com/embed/QIwFXTVn-5o',
	])
		assert.equal(getYouTubeVideoId(url), 'QIwFXTVn-5o')
})

test('rejects arbitrary iframe destinations and malformed video IDs', () => {
	for (const url of [
		'https://www.youtube.com.evil.test/embed/QIwFXTVn-5o',
		'https://www.youtube.com@evil.test/embed/QIwFXTVn-5o',
		'https://user:password@www.youtube.com/embed/QIwFXTVn-5o',
		'https://www.youtube.com:444/embed/QIwFXTVn-5o',
		'javascript:alert(1)',
		'https://www.youtube.com/embed/invalid',
		'https://www.youtube.com/embed/QIwFXTVn-5o/extra',
	])
		assert.equal(getYouTubeVideoId(url), undefined)
})

test('merges provider video metadata and description embeds without duplicate videos', () => {
	const media = getProjectGalleryMedia({
		gallery: [
			{ url: 'https://cdn.modrinth.com/banner.png', title: '__mc_server_banner__' },
			{
				url: 'https://media.forgecdn.net/preview.png',
				raw_url: 'https://media.forgecdn.net/original.png',
				title: 'Image',
			},
		],
		videos: [{ url: 'https://www.youtube.com/watch?v=QIwFXTVn-5o', title: 'Trailer 1' }],
		body: '<iframe src="https://www.youtube.com/embed/QIwFXTVn-5o"></iframe><iframe src="https://www.youtube.com/embed/-HqUkTtxtoU" title="Trailer 2"></iframe>',
	})
	assert.equal(media.length, 3)
	assert.equal(media[0].src, 'https://media.forgecdn.net/original.png')
	assert.equal(media[0].thumbnail, 'https://media.forgecdn.net/preview.png')
	assert.equal(media[1].title, 'Trailer 1')
	assert.equal(media[2].title, 'Trailer 2')
	assert.equal(media[2].src, 'https://www.youtube.com/watch?v=-HqUkTtxtoU')
})

test('ignores code samples and commented embeds rather than displaying them as videos', () => {
	assert.deepEqual(
		getProjectGalleryMedia({
			body: '<!-- <iframe src="https://www.youtube.com/embed/QIwFXTVn-5o"></iframe> --><pre><iframe src="https://www.youtube.com/embed/QIwFXTVn-5o"></iframe></pre><iframe src="https://evil.test/video"></iframe>',
		}),
		[],
	)
})
