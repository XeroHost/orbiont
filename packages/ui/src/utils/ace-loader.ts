import { createLazyLoader } from './lazy-load'

const loadEditor = createLazyLoader(async () => {
	const editor = await import('vue3-ace-editor')
	await import('./ace-theme')
	return editor.VAceEditor
})

// Literal imports let the bundler split modes without including workers or the resolver.
const modes = {
	text: createLazyLoader(() => import('ace-builds/src-noconflict/mode-text')),
	json: createLazyLoader(() => import('ace-builds/src-noconflict/mode-json')),
	toml: createLazyLoader(() => import('ace-builds/src-noconflict/mode-toml')),
	sh: createLazyLoader(() => import('ace-builds/src-noconflict/mode-sh')),
	batchfile: createLazyLoader(() => import('ace-builds/src-noconflict/mode-batchfile')),
	powershell: createLazyLoader(() => import('ace-builds/src-noconflict/mode-powershell')),
	yaml: createLazyLoader(() => import('ace-builds/src-noconflict/mode-yaml')),
	javascript: createLazyLoader(() => import('ace-builds/src-noconflict/mode-javascript')),
	typescript: createLazyLoader(() => import('ace-builds/src-noconflict/mode-typescript')),
	python: createLazyLoader(() => import('ace-builds/src-noconflict/mode-python')),
	ruby: createLazyLoader(() => import('ace-builds/src-noconflict/mode-ruby')),
	php: createLazyLoader(() => import('ace-builds/src-noconflict/mode-php')),
	html: createLazyLoader(() => import('ace-builds/src-noconflict/mode-html')),
	css: createLazyLoader(() => import('ace-builds/src-noconflict/mode-css')),
	java: createLazyLoader(() => import('ace-builds/src-noconflict/mode-java')),
	c_cpp: createLazyLoader(() => import('ace-builds/src-noconflict/mode-c_cpp')),
	rust: createLazyLoader(() => import('ace-builds/src-noconflict/mode-rust')),
	golang: createLazyLoader(() => import('ace-builds/src-noconflict/mode-golang')),
	markdown: createLazyLoader(() => import('ace-builds/src-noconflict/mode-markdown')),
	properties: createLazyLoader(() => import('ace-builds/src-noconflict/mode-properties')),
	ini: createLazyLoader(() => import('ace-builds/src-noconflict/mode-ini')),
	mclog: createLazyLoader(() => import('./ace-mode-log')),
	mcfunction: createLazyLoader(() => import('./ace-mode-mcfunction')),
}

export async function loadAceEditor(language: string) {
	const editor = await loadEditor()
	const mode = Object.hasOwn(modes, language) ? language : 'text'
	await modes[mode as keyof typeof modes]()
	return editor
}
