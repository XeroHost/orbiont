export interface BedrockStopOutcome {
	state: 'stopped' | 'timeout'
	force_token: string | null
}

/** A timeout offers a forced close only after the user explicitly confirms. */
export async function requestBedrockStop(
	stop: (forceToken?: string) => Promise<BedrockStopOutcome>,
	confirmForce: () => Promise<boolean>,
): Promise<BedrockStopOutcome> {
	const outcome = await stop()
	if (outcome.state !== 'timeout' || !outcome.force_token) return outcome
	if (!(await confirmForce())) return outcome
	return stop(outcome.force_token)
}
