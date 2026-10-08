import ace from 'ace-builds'

// Ace's AMD registration exists at runtime but is missing from its published declarations.
export const defineAceModule = (
	ace as unknown as {
		define: (
			id: string,
			dependencies: string[],
			factory: (
				require: (id: string) => unknown,
				exports: Record<string, unknown>,
				module: unknown,
			) => void,
		) => void
	}
).define
