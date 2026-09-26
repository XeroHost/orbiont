import type { AbstractApiClient } from './abstract-client'

export abstract class AbstractModule {
	protected client: AbstractApiClient

	public constructor(client: AbstractApiClient) {
		this.client = client
	}

	/**
	 * Get the module's name, used for error reporting & for module field generation.
	 * @returns Module name
	 */
	public abstract getModuleID(): string
}
