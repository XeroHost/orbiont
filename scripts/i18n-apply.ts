import chalk from 'chalk'
import * as fs from 'node:fs'
import * as path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

import { contractFromMessage, translationCompatibleWithSource } from './i18n-icu-contract.ts'

// Applies completed translation batches to local catalogs. Accepts the JSON
// from `i18n:gaps --out` with a `translation` field on each completed entry,
// or a compact `{ "<locale>": { "<key>": "<text>" } }` map. Validates known
// keys, nonempty text, and compatible ICU contracts before writing to the
// appropriate file and field, preserving the existing key order.

type MessageEntry = string | { message?: string; defaultMessage?: string }
type MessageFile = Record<string, MessageEntry>

type ScopeId = 'app' | 'ui'

interface ScopeDefinition {
	id: ScopeId
	/** Locale directory relative to the repository root. */
	directory: string
	/** Field containing message text in this catalog. */
	field: 'message' | 'defaultMessage'
}

const SCOPES: ScopeDefinition[] = [
	{ id: 'app', directory: 'apps/app-frontend/src/locales', field: 'message' },
	{ id: 'ui', directory: 'packages/ui/src/locales', field: 'defaultMessage' },
]

const SCOPE_BY_ID = new Map(SCOPES.map((scope) => [scope.id, scope]))
const SOURCE_LOCALE = 'en-US'

interface RawEntry {
	key?: unknown
	scope?: unknown
	translation?: unknown
	text?: unknown
	value?: unknown
	message?: unknown
	defaultMessage?: unknown
}

interface Requested {
	locale: string
	key: string
	scope?: ScopeId
	translation: string
}

interface Plan {
	scope: ScopeId
	locale: string
	key: string
	translation: string
	/** Original English text used to validate the ICU contract. */
	source: string
	/** Catalog file path relative to the repository root. */
	file: string
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function textOf(entry: MessageEntry | undefined): string | undefined {
	if (typeof entry === 'string') return entry
	return entry?.message ?? entry?.defaultMessage
}

function readMessageFile(file: string): MessageFile {
	if (!fs.existsSync(file)) return {}
	return JSON.parse(fs.readFileSync(file, 'utf8')) as MessageFile
}

/** Completed batch entry text, or `undefined` when no translation is provided. */
function translationOf(entry: RawEntry): string | undefined {
	for (const candidate of [
		entry.translation,
		entry.text,
		entry.value,
		entry.message,
		entry.defaultMessage,
	]) {
		if (typeof candidate === 'string') return candidate
	}
	return undefined
}

/**
 * Normalizes both supported batch formats into a list of requests.
 * Entries without text are skipped so partial batches can be applied.
 */
export function parseBatch(raw: unknown): Requested[] {
	if (!isPlainObject(raw)) throw new Error('El JSON debe ser un objeto')

	const requests: Requested[] = []

	if (isPlainObject(raw.locales)) {
		for (const [locale, value] of Object.entries(raw.locales)) {
			const entries = isPlainObject(value)
				? (value.entries as unknown[] | undefined)
				: Array.isArray(value)
					? value
					: undefined
			if (!Array.isArray(entries)) {
				throw new Error(`${locale}: el idioma no tiene una lista "entries"`)
			}
			for (const item of entries) {
				if (!isPlainObject(item)) throw new Error(`${locale}: entrada de lote inválida`)
				const key = item.key
				if (typeof key !== 'string' || key.length === 0) {
					throw new Error(`${locale}: entrada de lote sin clave`)
				}
				const translation = translationOf(item as RawEntry)
				if (translation === undefined) continue
				const scope = item.scope
				requests.push({
					locale,
					key,
					scope: scope === 'app' || scope === 'ui' ? scope : undefined,
					translation,
				})
			}
		}
		return requests
	}

	// Compact map: locale -> key -> text (or { translation, scope }).
	for (const [locale, value] of Object.entries(raw)) {
		if (!isPlainObject(value)) {
			throw new Error(`${locale}: se esperaba un objeto clave -> texto`)
		}
		for (const [key, item] of Object.entries(value)) {
			let translation: string | undefined
			let scope: ScopeId | undefined
			if (typeof item === 'string') {
				translation = item
			} else if (isPlainObject(item)) {
				translation = translationOf(item as RawEntry)
				if (item.scope === 'app' || item.scope === 'ui') scope = item.scope
			}
			if (translation === undefined) continue
			requests.push({ locale, key, scope, translation })
		}
	}

	return requests
}

type ScopeResolution = { scope: ScopeId } | { error: 'unknown' | 'ambiguous' }

function resolveScope(request: Requested, sources: Map<ScopeId, MessageFile>): ScopeResolution {
	const matches = SCOPES.filter((scope) => sources.get(scope.id)![request.key] !== undefined)
	if (request.scope) {
		return matches.some((scope) => scope.id === request.scope)
			? { scope: request.scope }
			: { error: 'unknown' }
	}
	if (matches.length === 1) return { scope: matches[0].id }
	return { error: matches.length > 1 ? 'ambiguous' : 'unknown' }
}

interface PlanResult {
	plans: Plan[]
	errors: string[]
}

/** Validates all requests and returns valid plans and errors. */
export function planTranslations(rootDir: string, requests: Requested[]): PlanResult {
	const sources = new Map<ScopeId, MessageFile>()
	for (const scope of SCOPES) {
		sources.set(
			scope.id,
			readMessageFile(path.join(rootDir, scope.directory, SOURCE_LOCALE, 'index.json')),
		)
	}

	const plans: Plan[] = []
	const seen = new Map<string, Plan>()
	const errors: string[] = []

	for (const request of requests) {
		const label = `${request.locale}:${request.key}`
		const resolution = resolveScope(request, sources)
		if ('error' in resolution) {
			errors.push(
				resolution.error === 'ambiguous'
					? `${label}: la clave existe en app y en ui; indica "scope"`
					: `${label}: la clave no existe en el catálogo en-US`,
			)
			continue
		}

		const scope = SCOPE_BY_ID.get(resolution.scope)!
		const file = path.join(scope.directory, request.locale, 'index.json')
		if (!fs.existsSync(path.join(rootDir, file))) {
			errors.push(`${label}: el idioma ${request.locale} no existe en el catálogo ${scope.id}`)
			continue
		}

		const sourceText = textOf(sources.get(scope.id)![request.key])
		if (!sourceText) {
			errors.push(`${label}: el inglés no tiene texto para esa clave`)
			continue
		}

		if (request.translation.trim() === '') {
			errors.push(`${label}: la traducción está vacía`)
			continue
		}

		let compatible = false
		try {
			compatible = translationCompatibleWithSource(
				contractFromMessage(sourceText, `${SOURCE_LOCALE}:${request.key}`),
				contractFromMessage(request.translation, `${request.locale}:${request.key}`),
			)
		} catch {
			compatible = false
		}
		if (!compatible) {
			errors.push(`${label}: el contrato ICU no coincide con el inglés`)
			continue
		}

		const id = `${scope.id}\u0000${request.locale}\u0000${request.key}`
		const previous = seen.get(id)
		if (previous) {
			if (previous.translation !== request.translation) {
				errors.push(`${label}: hay dos traducciones distintas para la misma clave`)
			}
			continue
		}

		const plan: Plan = {
			scope: scope.id,
			locale: request.locale,
			key: request.key,
			translation: request.translation,
			source: sourceText,
			file,
		}
		seen.set(id, plan)
		plans.push(plan)
	}

	return { plans, errors }
}

const simpleCompare = (left: string, right: string): number =>
	left < right ? -1 : left > right ? 1 : 0

function isSorted(keys: string[], compare: (left: string, right: string) => number): boolean {
	for (let index = 1; index < keys.length; index++) {
		if (compare(keys[index - 1], keys[index]) > 0) return false
	}
	return true
}

/**
 * Detects the file's sort order. If neither comparator matches, returns
 * `undefined` so new keys are appended without changing existing order.
 */
function detectComparator(keys: string[]): ((left: string, right: string) => number) | undefined {
	if (isSorted(keys, simpleCompare)) return simpleCompare
	if (isSorted(keys, (left, right) => left.localeCompare(right))) {
		return (left, right) => left.localeCompare(right)
	}
	return undefined
}

/** Inserts a key in sorted position without moving existing keys. */
function insertKey(
	keys: string[],
	key: string,
	compare: ((left: string, right: string) => number) | undefined,
) {
	if (keys.includes(key)) return
	if (!compare) {
		keys.push(key)
		return
	}
	const index = keys.findIndex((existing) => compare(existing, key) > 0)
	if (index === -1) keys.push(key)
	else keys.splice(index, 0, key)
}

interface ApplyResult {
	files: number
	keys: number
}

/** Writes plans to their files; `dryRun` leaves the filesystem unchanged. */
export function applyPlans(rootDir: string, plans: Plan[], dryRun: boolean): ApplyResult {
	const byFile = new Map<string, Plan[]>()
	for (const plan of plans) {
		const list = byFile.get(plan.file) ?? []
		list.push(plan)
		byFile.set(plan.file, list)
	}

	let files = 0
	let keys = 0

	for (const [file, entries] of byFile) {
		const absolute = path.join(rootDir, file)
		const messages = readMessageFile(absolute)
		const order = Object.keys(messages)
		const compare = detectComparator(order)

		for (const plan of entries) insertKey(order, plan.key, compare)

		const output: MessageFile = {}
		for (const key of order) output[key] = messages[key]
		for (const plan of entries) {
			output[plan.key] = { [SCOPE_BY_ID.get(plan.scope)!.field]: plan.translation }
		}

		if (!dryRun) fs.writeFileSync(absolute, `${JSON.stringify(output, null, 2)}\n`)
		files++
		keys += entries.length
	}

	return { files, keys }
}

function readOptions(args: string[]): Record<string, string | boolean> {
	const options: Record<string, string | boolean> = {}
	for (let index = 0; index < args.length; index++) {
		const arg = args[index]
		if (!arg.startsWith('-')) continue
		const key = arg.startsWith('--') ? arg.slice(2) : arg.slice(1)
		const equals = key.indexOf('=')
		if (equals !== -1) {
			options[key.slice(0, equals)] = key.slice(equals + 1)
			continue
		}
		if (key === 'in') {
			options.in = args[++index] ?? ''
			continue
		}
		options[key] = true
	}
	return options
}

const USAGE = `Uso: pnpm i18n:apply <fichero.json> [opciones]

Aplica un JSON de traducciones rellenas a los catálogos locales. Es el inverso
de \`i18n:gaps\`. Acepta el JSON de \`i18n:gaps --out\` con un campo
"translation" añadido a cada entrada, o un mapa compacto:
{ "<idioma>": { "<clave>": "<texto>" } }.

Escribe el texto en el fichero y el campo correctos (\`message\` en app,
\`defaultMessage\` en ui), conservando el orden de claves del fichero. Si
alguna entrada es inválida, no escribe nada y las enumera.

Opciones:
      --in <ruta>        Ruta del JSON (alternativa al argumento posicional)
      --dry-run          Comprueba y resume sin escribir
      --allow-partial    Aplica las entradas válidas aunque haya inválidas
      --json             Salida JSON con el resumen
      --help             Muestra esta ayuda
`

function main() {
	const args = process.argv.slice(2)
	const options = readOptions(args)

	if (options.help || options.h) {
		console.log(USAGE)
		return
	}

	const positional = args.find((arg) => !arg.startsWith('-'))
	const input = typeof options.in === 'string' && options.in ? options.in : positional
	if (!input) throw new Error('Falta la ruta del JSON de traducciones (usa --help)')

	const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
	const inputPath = path.resolve(rootDir, input)
	if (!fs.existsSync(inputPath)) throw new Error(`No existe el fichero ${input}`)

	const raw = JSON.parse(fs.readFileSync(inputPath, 'utf8')) as unknown
	const requests = parseBatch(raw)

	if (requests.length === 0) {
		console.log('No hay traducciones rellenas en el fichero.')
		return
	}

	const { plans, errors } = planTranslations(rootDir, requests)
	const dryRun = options['dry-run'] === true
	const allowPartial = options['allow-partial'] === true

	if (errors.length > 0) {
		for (const error of errors) console.error(chalk.red(`  ✗ ${error}`))
		if (!allowPartial) {
			throw new Error(`${errors.length} traducción(es) inválida(s); no se escribió nada`)
		}
	}

	if (plans.length === 0) {
		console.log(chalk.yellow('  No hay traducciones válidas que aplicar.'))
		return
	}

	const result = applyPlans(rootDir, plans, dryRun)

	const byLocale = new Map<string, { app: number; ui: number }>()
	for (const plan of plans) {
		const counts = byLocale.get(plan.locale) ?? { app: 0, ui: 0 }
		counts[plan.scope]++
		byLocale.set(plan.locale, counts)
	}

	if (options.json === true) {
		console.log(
			JSON.stringify(
				{
					dryRun,
					files: result.files,
					keys: result.keys,
					errors,
					locales: Object.fromEntries(
						[...byLocale.entries()].map(([locale, counts]) => [locale, counts]),
					),
				},
				null,
				2,
			),
		)
		return
	}

	console.log()
	console.log(chalk.bold.cyan(`  ◎ i18n Apply${dryRun ? chalk.gray(' (simulación)') : ''}`))
	console.log(chalk.gray(`  ${'─'.repeat(45)}`))
	console.log()
	for (const [locale, counts] of [...byLocale.entries()].sort(([left], [right]) =>
		left.localeCompare(right),
	)) {
		console.log(
			`  ${chalk.white.bold(locale)} ${chalk.gray('—')} ${chalk.cyan(String(counts.app).padStart(4))} ${chalk.gray('app')} ${chalk.magenta(String(counts.ui).padStart(4))} ${chalk.gray('ui')}`,
		)
	}
	console.log()
	console.log(chalk.gray(`  ${'─'.repeat(45)}`))
	console.log(
		`  ${chalk.gray('Ficheros')} ${chalk.white.bold(result.files)} ${chalk.gray('· Claves')} ${chalk.white.bold(result.keys)}${dryRun ? chalk.gray(' · sin escribir (--dry-run)') : ''}`,
	)
	console.log()
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
	try {
		main()
	} catch (error) {
		console.error(chalk.red(error instanceof Error ? error.message : String(error)))
		process.exit(1)
	}
}
