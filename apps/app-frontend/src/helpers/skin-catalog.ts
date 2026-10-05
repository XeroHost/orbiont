import { arrayBufferToBase64 } from '@orbiont/utils'
import { invoke } from '@tauri-apps/api/core'

import { determineModelType, normalize_skin_texture, type Skin } from './skins'

export type SkinProvider = 'default' | 'mineskin' | 'mcstat'
export type RemoteSkinProvider = Exclude<SkinProvider, 'default'>
export interface CatalogSkin {
	id: string
	name: string
	texture: string
	model: 'classic' | 'slim' | 'unknown'
	tags: string[]
}
export interface CatalogPage {
	provider: RemoteSkinProvider
	skins: CatalogSkin[]
	page: number
	next: string | null
}
export interface CatalogFilters {
	page?: number
	after?: string
	search?: string
	model?: string
	sort?: string
	tag?: string
}

const cooldowns = new Map<RemoteSkinProvider, number>()

export async function getSkinCatalog(
	provider: RemoteSkinProvider,
	filters: CatalogFilters,
): Promise<CatalogPage> {
	const remaining = Math.ceil(((cooldowns.get(provider) || 0) - Date.now()) / 1000)
	if (remaining > 0) throw new Error(`Skin catalog ${provider}: 429; Retry-After: ${remaining}`)
	const query = Object.entries(filters)
		.filter(([, value]) => value !== undefined && value !== '')
		.map(([key, value]) => [key, String(value)])
	try {
		return await invoke('plugin:minecraft-skins|get_skin_catalog', { provider, query })
	} catch (error) {
		const message = String(error)
		if (message.includes(': 429')) {
			const seconds = Number(/Retry-After: (\d+)/.exec(message)?.[1] || 60)
			cooldowns.set(provider, Date.now() + Math.max(1, seconds) * 1000)
		}
		throw error
	}
}

export function catalogSkin(item: CatalogSkin, provider: RemoteSkinProvider): Skin {
	return {
		texture_key: `catalog:${provider}:${item.id}`,
		name: item.name || item.id.slice(0, 8),
		section: 'catalog',
		variant: item.model === 'slim' ? 'SLIM' : item.model === 'classic' ? 'CLASSIC' : 'UNKNOWN',
		texture: item.texture,
		source: 'custom_external',
		is_equipped: false,
	}
}

const textures = new Map<string, Promise<Uint8Array>>()
export function getCatalogTexture(url: string): Promise<Uint8Array> {
	let pending = textures.get(url)
	if (!pending) {
		if (textures.size >= 64) textures.delete(textures.keys().next().value!)
		pending = invoke<number[]>('plugin:minecraft-skins|get_catalog_skin_texture', { url })
			.then((bytes) => normalize_skin_texture(new Uint8Array(bytes)))
			.catch((error) => {
				textures.delete(url)
				throw error
			})
		textures.set(url, pending)
	}
	return pending
}

export async function prepareCatalogSkin(skin: Skin): Promise<{ skin: Skin; bytes: Uint8Array }> {
	const bytes = await getCatalogTexture(skin.texture)
	const texture = `data:image/png;base64,${arrayBufferToBase64(bytes)}`
	return {
		bytes,
		skin: {
			...skin,
			texture,
			variant: skin.variant === 'UNKNOWN' ? await determineModelType(texture) : skin.variant,
		},
	}
}
