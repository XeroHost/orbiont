const trimTrailingSlash = (url: string) => url.replace(/\/$/, '')

const siteUrl = trimTrailingSlash(import.meta.env.MODRINTH_URL || 'https://modrinth.com')
const labrinthBaseUrl = trimTrailingSlash(
	import.meta.env.MODRINTH_API_BASE_URL || 'https://api.modrinth.com',
)

// Update manifest for the Linux "new version" notice (other platforms use
// Tauri's updater, configured in apps/app/tauri-release.conf.json).
const updatesUrl =
	import.meta.env.ORBIONT_UPDATES_URL ||
	'https://github.com/XeroHost/orbiont/releases/latest/download/updates.json'

export const config = {
	updatesUrl,
	siteUrl,
	labrinthBaseUrl,
}
