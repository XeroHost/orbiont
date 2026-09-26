import config from '@orbiont/tooling-config/eslint/nuxt.mjs'

export default config.append([
	{
		ignores: ['src/generated/app-events/*.ts', 'src/generated/app-events/postcard/**'],
	},
	{
		rules: {
			'turbo/no-undeclared-env-vars': ['error', { allowList: ['^DEV$', '^PROD$'] }],
		},
	},
	{
		// App.vue and other plain-JS components aren't type-checked, so an
		// identifier left behind after removing a feature only fails at
		// runtime (and a crash in App.vue keeps the window hidden).
		files: ['src/**/*.js', 'src/**/*.vue'],
		ignores: ['src/**/*.ts'],
		rules: {
			'no-undef': 'error',
		},
	},
])
