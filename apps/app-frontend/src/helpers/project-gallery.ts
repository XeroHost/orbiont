type GalleryImage = {
	url: string
	raw_url?: string
	title?: string
	description?: string
	created?: string
}

type GalleryVideo = { url: string; title?: string; description?: string }

export type ProjectGalleryMedia = {
	id: string
	src: string
	thumbnail: string
	title?: string
	description?: string
	created?: string
	youtubeVideoId?: string
}

export function getYouTubeVideoId(source: string): string | undefined {
	try {
		const url = new URL(source.replaceAll('&amp;', '&'), 'https://www.youtube.com')
		if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password || url.port) {
			return undefined
		}
		let id: string | undefined | null
		if (url.hostname === 'youtu.be') {
			id = url.pathname.slice(1)
		} else if (
			[
				'youtube.com',
				'www.youtube.com',
				'm.youtube.com',
				'youtube-nocookie.com',
				'www.youtube-nocookie.com',
			].includes(url.hostname)
		) {
			if (url.pathname === '/watch') id = url.searchParams.get('v')
			else id = /^\/(?:embed|shorts|live)\/([^/]+)$/.exec(url.pathname)?.[1]
		}
		return id && /^[A-Za-z0-9_-]{11}$/.test(id) ? id : undefined
	} catch {
		return undefined
	}
}

export function getProjectGalleryMedia(project: {
	gallery?: GalleryImage[]
	videos?: GalleryVideo[]
	body?: string
}): ProjectGalleryMedia[] {
	const media: ProjectGalleryMedia[] = []
	const videoIds = new Set<string>()
	function addVideo(url: string, title?: string, description?: string) {
		const id = getYouTubeVideoId(url)
		if (!id || videoIds.has(id)) return
		videoIds.add(id)
		media.push({
			id: `youtube-${id}`,
			src: `https://www.youtube.com/watch?v=${id}`,
			thumbnail: `https://i.ytimg.com/vi/${id}/hqdefault.jpg`,
			title: title || 'YouTube',
			description,
			youtubeVideoId: id,
		})
	}
	for (const image of project.gallery ?? []) {
		if (image.title === '__mc_server_banner__') continue
		const src = image.raw_url ?? image.url
		if (getYouTubeVideoId(src)) addVideo(src, image.title, image.description)
		else media.push({ ...image, id: image.url, src, thumbnail: image.url })
	}
	for (const video of project.videos ?? []) addVideo(video.url, video.title, video.description)
	// Read only video URLs, never inject the author's HTML into the viewer.
	const description = (project.body ?? '')
		.replace(/<!--[\s\S]*?-->/g, '')
		.replace(/<(script|style|pre|code)\b[\s\S]*?<\/\1\s*>/gi, '')
	for (const tag of description.matchAll(/<iframe\b[^>]*>/gi)) {
		const source = /\ssrc\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'<>]+))/i.exec(tag[0])
		const title = /\stitle\s*=\s*(?:"([^"]*)"|'([^']*)')/i.exec(tag[0])
		if (source) addVideo(source[1] ?? source[2] ?? source[3], title?.[1] ?? title?.[2])
	}
	return media
}
