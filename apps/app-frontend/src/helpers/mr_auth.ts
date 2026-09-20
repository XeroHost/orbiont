/**
 * Orbiont doesn't ship Modrinth account sign-in (see Fase 1 of the build
 * plan: packages/app-lib/src/api/mr_auth.rs and state/mr_auth.rs are gone).
 * These stay as no-ops, returning "signed out", so call sites throughout
 * the app that still reference a Modrinth account don't need to change.
 */

export type ModrinthCredentials = {
	session: string
	expires: string
	user_id: string
	active: boolean
}

export type ModrinthAuthFlow = 'sign-in' | 'sign-up'

export async function login(
	_flow: ModrinthAuthFlow = 'sign-in',
	_addAccount = false,
): Promise<ModrinthCredentials> {
	throw new Error('Modrinth account sign-in is not available')
}

export async function logout(): Promise<void> {}

export async function get(): Promise<ModrinthCredentials | null> {
	return null
}

export async function getAll(): Promise<ModrinthCredentials[]> {
	return []
}

export async function setActive(_userId: string): Promise<void> {}

export async function removeUser(_userId: string): Promise<void> {}

export async function cancelLogin(): Promise<void> {}
