import type { BedrockStatus } from './bedrock'

export async function removeBedrockBatch<T>(
	items: T[],
	remove: (item: T) => Promise<unknown>,
	invalidate: () => Promise<unknown>,
) {
	if (!items.length) return
	try {
		for (const item of items) await remove(item)
	} finally {
		await invalidate()
	}
}

/** Export only known non-personal facts; raw native errors can contain usernames and paths. */
export function bedrockDiagnosticReport(status: BedrockStatus) {
	const version = (value: string) => (/^[0-9]+(?:\.[0-9]+){1,4}$/.test(value) ? value : 'unknown')
	const states = ['idle', 'requested', 'starting', 'running', 'timeout', 'failed']
	return {
		edition: 'bedrock',
		supported: status.supported,
		game_running: status.game_running,
		game: status.game
			? { version: version(status.game.version), can_launch: status.game.can_launch }
			: null,
		launcher: status.launcher
			? { version: version(status.launcher.version), can_launch: status.launcher.can_launch }
			: null,
		launch: status.launch
			? {
					state: states.includes(status.launch.state) ? status.launch.state : 'unknown',
					elapsed_ms: Number.isFinite(status.launch.elapsed_ms)
						? Math.max(0, status.launch.elapsed_ms)
						: 0,
				}
			: null,
	}
}
