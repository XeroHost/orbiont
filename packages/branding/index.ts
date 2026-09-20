/**
 * Single source of truth for Orbiont's brand strings and colors.
 *
 * "Orbiont" is a placeholder codename (see the build plan's Fase 5):
 * it collides with an existing UK company and is awkward to spell in
 * Spanish. Everything that identifies the product reads from here so
 * that renaming later is "change six lines", not a repo-wide search
 * and replace. Never hardcode "Orbiont", the bundle identifier, the
 * deep-link scheme, or the domain anywhere else.
 */

export const productName = 'Orbiont'

/** Reverse-DNS app identifier used in tauri.conf.json and platform manifests. */
export const bundleIdentifier = 'com.xerohost.orbiont'

/** Custom URL scheme registered for deep links (e.g. orbiont://...). */
export const deepLinkScheme = 'orbiont'

/**
 * Placeholder domain — not purchased yet. The build plan calls for five
 * checks (domain, Modrinth/CurseForge, GitHub, trademark/USPTO, socials)
 * before the name and domain are final in Fase 5.
 */
export const domain = 'orbiont.gg'
export const siteUrl = `https://${domain}`
export const supportEmail = `support@${domain}`

/**
 * Brand colors, mirroring the --color-cyan-* / --color-brand-* tokens in
 * packages/assets/styles/variables.scss. Use the CSS variables in Vue/CSS;
 * reach for these constants only where a CSS custom property isn't
 * available (native code, generated manifests, image generation scripts).
 */
export const brandColors = {
	cyan300: '#00ffff',
	cyan400: '#00f0f0',
	cyan500: '#00e1e1',
	cyan600: '#00c3c3',
	cyan700: '#00a7a7',
	cyan800: '#008b8b',
	/** Brand shade used against the dark theme's background. */
	brandDark: '#00c3c3',
	/** Brand shade used against the light theme's background. */
	brandLight: '#00a7a7',
} as const
