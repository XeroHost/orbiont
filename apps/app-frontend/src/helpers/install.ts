import { invoke } from '@tauri-apps/api/core'

import type { InstallErrorView } from '@/generated/app-events/InstallErrorView'
import type { InstallJavaStep } from '@/generated/app-events/InstallJavaStep'
import type { InstallJobSnapshot } from '@/generated/app-events/InstallJobSnapshot'
import type { InstallJobStatus } from '@/generated/app-events/InstallJobStatus'
import type { InstallPhaseId } from '@/generated/app-events/InstallPhaseId'
import type { InstallProgress } from '@/generated/app-events/InstallProgress'
import type { InstallProgressSecondary } from '@/generated/app-events/InstallProgressSecondary'
import type { AppEvents } from '@/providers/app-events'

import { convertCurseforgeModpack, downloadCurseforgeModpack, isCurseforgeId } from './curseforge'
import { requestOptifineForPack } from './optifine'
import type { OptifineReference } from './optifine-selection'
import type { InstanceIconConfig, InstanceLink, InstanceLoader } from './types'

export type {
	InstallErrorView,
	InstallJavaStep,
	InstallJobSnapshot,
	InstallJobStatus,
	InstallPhaseId,
	InstallProgress,
	InstallProgressSecondary,
}

export interface PackLocationVersionId {
	type: 'fromVersionId'
	project_id: string
	version_id: string
	title: string
	icon_url?: string | null
}

export interface PackLocationFile {
	type: 'fromFile'
	path: string
}

export type CreatePackLocation = PackLocationVersionId | PackLocationFile

export interface InstallModpackPreview {
	optifine?: OptifineReference | null
	name: string
	gameVersion: string
	modloader: InstanceLoader
	loaderVersion: string | null
	icon?: string | null
	iconUrl?: string | null
	link?: InstanceLink | null
	unknownFile: boolean
	externalFilesInModpack: string[]
}

export interface InstallCreateInstanceRequest {
	optifineInstallerPath?: string | null
	name: string
	gameVersion: string
	loader: InstanceLoader
	loaderVersion: string | null
	iconPath: string | null
	iconConfig?: InstanceIconConfig | null
	link?: InstanceLink | null
}

export interface InstallPostInstallEdit {
	name?: string | null
	iconPath?: string | null
	link?: InstanceLink | null
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null
}

export function getErrorMessage(error: unknown): string {
	if (typeof error === 'string') return error
	if (error instanceof Error) return error.message || 'Unknown error'
	if (isRecord(error) && typeof error.message === 'string') return error.message
	return 'Unknown error'
}

/**
 * CurseForge modpack versions install from their downloaded file, converted
 * to an .mrpack. Only the real install reports files that had to be left out.
 */
async function resolvePackLocation(
	location: CreatePackLocation,
	reportSkipped: boolean,
): Promise<CreatePackLocation> {
	if (location.type === 'fromVersionId' && isCurseforgeId(location.version_id)) {
		const path = await downloadCurseforgeModpack(
			location.version_id,
			reportSkipped ? location.title : undefined,
		)
		return { type: 'fromFile', path }
	}
	if (location.type === 'fromFile' && /\.zip$/i.test(location.path)) {
		const packName = location.path.split(/[\\/]/).pop() ?? location.path
		const path = await convertCurseforgeModpack(location.path, reportSkipped ? packName : undefined)
		return { type: 'fromFile', path }
	}
	return location
}

export async function install_get_modpack_preview(location: CreatePackLocation) {
	location = await resolvePackLocation(location, false)
	return await invoke<InstallModpackPreview>('plugin:install|install_get_modpack_preview', {
		location,
	})
}

export async function install_create_instance(request: InstallCreateInstanceRequest) {
	return await invoke<InstallJobSnapshot>('plugin:install|install_create_instance', { request })
}

export async function install_create_modpack_instance(
	location: CreatePackLocation,
	postInstallEdit?: InstallPostInstallEdit | null,
) {
	if (location.type === 'fromVersionId' && isCurseforgeId(location.version_id)) {
		postInstallEdit ??= { name: location.title }
	}
	location = await resolvePackLocation(location, true)
	const optifineInstallerPath = await requestOptifineForPack(location)
	return await invoke<InstallJobSnapshot>('plugin:install|install_create_modpack_instance', {
		location,
		postInstallEdit,
		optifineInstallerPath,
	})
}

export async function install_import_instance(
	launcherType: string,
	basePath: string,
	instanceFolder: string,
) {
	return await invoke<InstallJobSnapshot>('plugin:install|install_import_instance', {
		launcherType,
		basePath,
		instanceFolder,
	})
}

export async function install_duplicate_instance(sourceInstanceId: string) {
	return await invoke<InstallJobSnapshot>('plugin:install|install_duplicate_instance', {
		sourceInstanceId,
	})
}

export async function install_existing_instance(instanceId: string, force: boolean) {
	return await invoke<InstallJobSnapshot>('plugin:install|install_existing_instance', {
		instanceId,
		force,
	})
}

export async function install_pack_to_existing_instance(
	instanceId: string,
	location: CreatePackLocation,
	postInstallEdit?: InstallPostInstallEdit | null,
) {
	location = await resolvePackLocation(location, true)
	const optifineInstallerPath = await requestOptifineForPack(location)
	return await invoke<InstallJobSnapshot>('plugin:install|install_pack_to_existing_instance', {
		instanceId,
		location,
		postInstallEdit,
		optifineInstallerPath,
	})
}

export async function install_bulk_update_content(
	instanceId: string,
	updates: { project_path: string; version_id: string }[],
) {
	return await invoke<InstallJobSnapshot>('plugin:install|install_bulk_update_content', {
		instanceId,
		updates,
	})
}

export async function install_job_list(includeFinished: boolean) {
	return await invoke<InstallJobSnapshot[]>('plugin:install|install_job_list', { includeFinished })
}

export async function install_job_get(jobId: string) {
	return await invoke<InstallJobSnapshot>('plugin:install|install_job_get', { jobId })
}

export async function install_job_retry(jobId: string) {
	return await invoke<InstallJobSnapshot>('plugin:install|install_job_retry', { jobId })
}

export async function install_job_cancel(jobId: string) {
	return await invoke<InstallJobSnapshot>('plugin:install|install_job_cancel', { jobId })
}

export async function install_job_pause(jobId: string) {
	return await invoke<InstallJobSnapshot>('plugin:install|install_job_pause', { jobId })
}

export async function install_job_resume(jobId: string) {
	return await invoke<InstallJobSnapshot>('plugin:install|install_job_resume', { jobId })
}

export async function install_job_dismiss(jobId: string) {
	return await invoke<void>('plugin:install|install_job_dismiss', { jobId })
}

export async function install_job_support_details(jobId: string) {
	return await invoke<string>('plugin:install|install_job_support_details', { jobId })
}

export function installJobInstanceId(job: InstallJobSnapshot): string | null {
	return job.instance_id ?? job.target.instance_id ?? null
}

export function isInstallJobFinished(status: InstallJobStatus) {
	return (
		status === 'succeeded' ||
		status === 'failed' ||
		status === 'interrupted' ||
		status === 'canceled'
	)
}

function settleInstallJob(job: InstallJobSnapshot) {
	if (job.status === 'succeeded') return job

	if (job.error) throw job.error
	throw new Error(`Install job ${job.job_id} ${job.status}`)
}

export async function wait_for_install_job(events: AppEvents, jobId: string) {
	const current = await install_job_get(jobId)
	if (isInstallJobFinished(current.status)) return settleInstallJob(current)

	return await new Promise<InstallJobSnapshot>((resolve, reject) => {
		let finished = false
		let unlisten: (() => void) | null = null

		const cleanup = () => {
			if (unlisten) {
				unlisten()
				unlisten = null
			}
		}

		const resolveJob = (job: InstallJobSnapshot) => {
			if (finished || job.job_id !== jobId || !isInstallJobFinished(job.status)) return

			finished = true
			cleanup()

			try {
				resolve(settleInstallJob(job))
			} catch (err) {
				reject(err)
			}
		}

		const rejectWait = (err: unknown) => {
			if (finished) return
			finished = true
			cleanup()
			reject(err)
		}

		unlisten = events.on('install_job', resolveJob)
		install_job_get(jobId).then(resolveJob).catch(rejectWait)
	})
}

export async function validate_bulk_update_content(
	instanceId: string,
	updates: { project_path: string; version_id: string }[],
): Promise<void> {
	await invoke('plugin:install|validate_bulk_update_content', { instanceId, updates })
}
