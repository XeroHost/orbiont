import { invoke } from '@tauri-apps/api/core'

export type CatalogSource = 'xerohost' | 'modrinth' | 'curseforge'

export interface Modpack {
	id: string
	name: string
	description: string
	icon: string
	gameVersion: string
	loader: string
	downloadUrl: string
	hash: string
	source: CatalogSource
}

export interface OrbiontServer {
	id: string
	name: string
	host: string
	port: number
	icon: string
	tags: string[]
	featured: boolean
	modpackId: string | null
}

export async function getModpacks(): Promise<Modpack[]> {
	return await invoke('plugin:orbiont|orbiont_get_modpacks')
}

export async function getServers(): Promise<OrbiontServer[]> {
	return await invoke('plugin:orbiont|orbiont_get_servers')
}

/** Downloads a catalog modpack's .mrpack to a local cache path for installing. */
export async function downloadModpack(modpack: Modpack): Promise<string> {
	return await invoke('plugin:orbiont|orbiont_download_modpack', { modpack })
}
