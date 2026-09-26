export type ServerLoader =
	| 'Forge'
	| 'NeoForge'
	| 'Fabric'
	| 'Quilt'
	| 'Purpur'
	| 'Spigot'
	| 'Vanilla'
	| 'Paper'
	| 'Bukkit'

export const loaderDisplayNames: Record<string, string> = {
	fabric: 'Fabric',
	neoforge: 'NeoForge',
	neo_forge: 'NeoForge',
	forge: 'Forge',
	quilt: 'Quilt',
	paper: 'Paper',
	spigot: 'Spigot',
	purpur: 'Purpur',
	bukkit: 'Bukkit',
	vanilla: 'Vanilla',
}

export const formatLoaderLabel = (item: string) =>
	loaderDisplayNames[item] ?? item.charAt(0).toUpperCase() + item.slice(1)
