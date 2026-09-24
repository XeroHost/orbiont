/**
 * Orbiont doesn't ship ads (see Fase 1 of the build plan: apps/app/src/api/ads.rs
 * and apps/app/src/api/ads-consent/** are gone). These stay as no-ops so call
 * sites throughout the app don't need to change.
 */

export async function init_ads_window() {}

export async function take_ads_window_hold() {}

export async function release_ads_window_hold() {}

export async function hide_ads_window() {}

export async function should_show_ads_consent_popup() {
	return false
}

export async function perform_ads_consent_action() {}

export async function open_ads_consent_preferences() {}

export async function record_ads_click() {}

export async function open_ads_link() {}
