/**
 * Single source of truth for Orbiont's brand strings and colors.
 *
 * Keep the product name, bundle identifier, deep-link scheme, and domain
 * here so that every consumer uses the same identity.
 */

export const productName = 'Orbiont'

/**
 * App identifier used in tauri.conf.json and platform manifests, and as the
 * name of the app's data directory (see packages/app-lib/src/state/dirs.rs).
 * Modrinth's own app uses a plain "ModrinthApp" here rather than a
 * reverse-DNS id, so this follows the same pattern.
 */
export const bundleIdentifier = 'Orbiont'

/** Custom URL scheme registered for deep links (e.g. orbiont://...). */
export const deepLinkScheme = 'orbiont'

/**
 * Orbiont has no infrastructure of its own: its site, API and updates live
 * under XeroHost's domain (xerohost.net/orbiont). Keep in sync with the
 * ORBIONT_* values in packages/app-lib/.env.* (read by the Rust side).
 */
export const domain = 'xerohost.net'
export const companyName = 'XeroHost'
export const companySiteUrl = `https://${domain}`
export const siteUrl = `https://${domain}/orbiont`
export const supportEmail = `support@${domain}`
export const supportUrl = `${siteUrl}/support`
export const changelogUrl = `${siteUrl}/changelog`

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
