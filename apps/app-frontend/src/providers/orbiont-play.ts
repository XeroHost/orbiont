import {
	install_create_modpack_instance,
	installJobInstanceId,
	wait_for_install_job,
} from '@/helpers/install'
import { get, list } from '@/helpers/instance'
import type { Modpack, OrbiontServer, SearchResult } from '@/helpers/orbiont'
import { downloadModpack } from '@/helpers/orbiont'
import type { GameInstance } from '@/helpers/types'
import { ensureManagedServerWorldExists, start_join_server } from '@/helpers/worlds'
import type { AppEvents } from '@/providers/app-events'

/** Finds the instance (if any) that was installed from this catalog modpack. */
export async function findInstanceForModpack(modpackId: string): Promise<GameInstance | null> {
	const instances = await list()
	return (
		instances.find(
			(instance) =>
				instance.link?.type === 'imported_modpack' && instance.link.project_id === modpackId,
		) ?? null
	)
}

/**
 * "Play in one click": installs the server's modpack if it isn't already
 * installed, registers the server on that instance's world list, and
 * launches straight into it with quick-play. See the build plan, Fase 4.
 *
 * Throws on failure — the three error states the plan calls out (server
 * down, download failed, user not logged in) all surface as a thrown error
 * for the caller to route through `handleSevereError`, which already shows
 * the right modal for "not logged in" (see composables/use-error.js) and a
 * generic error otherwise.
 */
export async function playOrbiontServer(
	server: OrbiontServer,
	modpack: Modpack | null,
	appEvents: AppEvents,
): Promise<{ instanceId: string }> {
	const address = `${server.host}:${server.port}`

	let instance = server.modpackId ? await findInstanceForModpack(server.modpackId) : null

	if (!instance) {
		if (!modpack) {
			throw new Error(`${server.name} doesn't have a modpack configured yet.`)
		}

		let path: string
		try {
			path = await downloadModpack(modpack)
		} catch (err) {
			throw new Error(`Couldn't download ${modpack.name}: ${(err as Error).message ?? err}`)
		}

		const job = await install_create_modpack_instance(
			{ type: 'fromFile', path },
			{
				name: modpack.name,
				link: { type: 'imported_modpack', project_id: modpack.id },
			},
		)
		const instanceId = installJobInstanceId(job)
		if (!instanceId) {
			throw new Error(`Installing ${modpack.name} didn't produce an instance.`)
		}

		await wait_for_install_job(appEvents, job.job_id)

		instance = await get(instanceId)
		if (!instance) {
			throw new Error(`${modpack.name} installed, but the instance couldn't be found afterward.`)
		}
	}

	await ensureManagedServerWorldExists(instance.id, server.name, address)

	// start_join_server surfaces "User is not logged in" when there's no
	// default Microsoft/Minecraft account — handleSevereError already shows
	// the sign-in modal for that message (composables/use-error.js).
	await start_join_server(instance.id, address)

	return { instanceId: instance.id }
}

/**
 * Installs a modpack file already saved to a local cache path (via
 * `downloadSearchResult` or `saveDroppedFile`) as a new instance. Used by the
 * search UI's install buttons — see the build plan, Fase 4.
 */
export async function installSearchResultFile(
	result: SearchResult,
	filePath: string,
	appEvents: AppEvents,
): Promise<{ instanceId: string }> {
	const job = await install_create_modpack_instance(
		{ type: 'fromFile', path: filePath },
		{ name: result.name },
	)
	const instanceId = installJobInstanceId(job)
	if (!instanceId) {
		throw new Error(`Installing ${result.name} didn't produce an instance.`)
	}

	await wait_for_install_job(appEvents, job.job_id)

	return { instanceId }
}
