// Vite exposes matching variables to browser code. Keep private signing,
// provider keys and OAuth tokens outside these public configuration names.
export const publicEnvPrefixes = [
	'VITE_VUE_SCAN',
	'MODRINTH_URL',
	'MODRINTH_API_BASE_URL',
	'ORBIONT_UPDATES_URL',
]
