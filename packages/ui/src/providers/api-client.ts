import type { AbstractApiClient } from '@orbiont/api-client'

import { createContext } from './create-context'

export const [injectApiClient, provideApiClient] = createContext<AbstractApiClient>(
	'root',
	'modrinthClient',
)
