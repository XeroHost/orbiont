import chalk from 'chalk'
import * as fs from 'node:fs'
import * as path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

import { contractFromMessage, translationCompatibleWithSource } from './i18n-icu-contract.ts'

// Lists missing translations and incompatible ICU contracts by locale,
// together with their English source text. Use --limit/--offset or --out
// to work in batches. --hint adds existing translations from related locales
// (for example, --hint ru-RU when translating uk-UA).

type MessageEntry = string | { message?: string; defaultMessage?: string }
type MessageFile = Record<string, MessageEntry>

type ScopeId = 'app' | 'ui'

interface ScopeDefinition {
	id: ScopeId
	/** Locale directory relative to the repository root. */
	directory: string
}

const SCOPES: ScopeDefinition[] = [
	{ id: 'app', directory: 'apps/app-frontend/src/locales' },
	{ id: 'ui', directory: 'packages/ui/src/locales' },
]

const SOURCE_LOCALE = 'en-US'
const DEFAULT_LIMIT = 25

export interface PendingMessage {
	key: string
	scope: ScopeId
	/** Locale file where the translation belongs. */
	file: string
	/** Reason this entry counts as pending. */
	reason: 'missing' | 'incompatible'
	/** Original English text. */
	source: string
	/** Reference text by related locale, provided only with --hint. */
	hints?: Record<string, string>
}

export interface LocaleGaps {
	locale: string
	totalMessages: number
	translatedMessages: number
	pendingMessages: number
	missing: number
	incompatible: number
	/** Keys selected after applying --offset/--limit. */
	entries: PendingMessage[]
	/** Pending entries outside the current selection. */
	remaining: number
}

export interface CollectGapsOptions {
	locales?: string[]
	scope?: ScopeId
	offset?: number
	limit?: number
	/** Includes complete locales with no pending entries. */
	includeComplete?: boolean
	/** Related locales that provide reference translations for each gap. */
	hintLocales?: string[]
}

function textOf(entry: MessageEntry | undefined): string | undefined {
	if (typeof entry === 'string') return entry
	return entry?.message ?? entry?.defaultMessage
}

function readMessageFile(file: string): MessageFile {
	if (!fs.existsSync(file)) return {}
	return JSON.parse(fs.readFileSync(file, 'utf8')) as MessageFile
}

function relativePath(rootDir: string, file: string): string {
	return path.relative(rootDir, file).split(path.sep).join('/')
}

function localeCodes(rootDir: string): string[] {
	const codes = new Set<string>()
	for (const scope of SCOPES) {
		const directory = path.join(rootDir, scope.directory)
		if (!fs.existsSync(directory)) continue
		for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
			if (entry.isDirectory() && fs.existsSync(path.join(directory, entry.name, 'index.json'))) {
				codes.add(entry.name)
			}
		}
	}
	return [...codes].sort((left, right) => left.localeCompare(right))
}

/**
 * Merges a locale in the same order used for Settings coverage:
 * app-frontend first, then UI, so the counts agree.
 */
function mergedCatalog(rootDir: string, locale: string): MessageFile {
	const merged: MessageFile = {}
	for (const scope of SCOPES) {
		Object.assign(
			merged,
			readMessageFile(path.join(rootDir, scope.directory, locale, 'index.json')),
		)
	}
	return merged
}

const SCOPE_ORDER: ScopeId[] = SCOPES.map((scope) => scope.id)

function compareEntries(left: PendingMessage, right: PendingMessage): number {
	const byScope = SCOPE_ORDER.indexOf(left.scope) - SCOPE_ORDER.indexOf(right.scope)
	if (byScope !== 0) return byScope
	if (left.key === right.key) return 0
	return left.key < right.key ? -1 : 1
}

/** Missing translations by locale with their English source text. */
export function collectGaps(rootDir: string, options: CollectGapsOptions = {}): LocaleGaps[] {
	const wanted = options.locales?.map((locale) => locale.trim()).filter(Boolean) ?? []
	const offset = options.offset ?? 0
	const limit = options.limit ?? Number.POSITIVE_INFINITY
	const available = localeCodes(rootDir)

	const sources: [ScopeDefinition, MessageFile][] = SCOPES.map((scope) => [
		scope,
		readMessageFile(path.join(rootDir, scope.directory, SOURCE_LOCALE, 'index.json')),
	])

	// Validate and load related locales once.
	const hintCatalogs = new Map<string, MessageFile>()
	for (const hint of options.hintLocales?.map((locale) => locale.trim()).filter(Boolean) ?? []) {
		if (hint.toLowerCase() === SOURCE_LOCALE.toLowerCase()) {
			throw new Error(`--hint: ${SOURCE_LOCALE} es el idioma fuente, no sirve de referencia`)
		}
		const code = available.find((entry) => entry.toLowerCase() === hint.toLowerCase())
		if (!code) throw new Error(`--hint: el idioma ${hint} no existe en los catálogos locales`)
		hintCatalogs.set(code, mergedCatalog(rootDir, code))
	}

	const results: LocaleGaps[] = []

	for (const locale of available) {
		if (locale === SOURCE_LOCALE) continue
		if (
			wanted.length > 0 &&
			!wanted.some((entry) => entry.toLowerCase() === locale.toLowerCase())
		) {
			continue
		}

		const translations = mergedCatalog(rootDir, locale)

		/** Existing translation of this key in each requested related locale. */
		const hintsFor = (key: string): Record<string, string> | undefined => {
			if (hintCatalogs.size === 0) return undefined
			const hints: Record<string, string> = {}
			for (const [code, catalog] of hintCatalogs) {
				if (code === locale) continue
				const text = textOf(catalog[key])
				if (text) hints[code] = text
			}
			return Object.keys(hints).length > 0 ? hints : undefined
		}

		const pending: PendingMessage[] = []
		let totalMessages = 0
		let translatedMessages = 0
		let incompatible = 0

		for (const [scope, source] of sources) {
			if (options.scope && options.scope !== scope.id) continue

			for (const [key, entry] of Object.entries(source)) {
				const sourceText = textOf(entry)
				if (!sourceText) continue
				totalMessages++

				const file = `${scope.directory}/${locale}/index.json`
				const translationText = textOf(translations[key])

				if (!translationText) {
					pending.push({
						key,
						scope: scope.id,
						file,
						reason: 'missing',
						source: sourceText,
						hints: hintsFor(key),
					})
					continue
				}

				let compatible = false
				try {
					compatible = translationCompatibleWithSource(
						contractFromMessage(sourceText, `${SOURCE_LOCALE}:${key}`),
						contractFromMessage(translationText, `${locale}:${key}`),
					)
				} catch {
					compatible = false
				}

				if (compatible) {
					translatedMessages++
					continue
				}

				incompatible++
				pending.push({
					key,
					scope: scope.id,
					file,
					reason: 'incompatible',
					source: sourceText,
					hints: hintsFor(key),
				})
			}
		}

		pending.sort(compareEntries)
		if (pending.length === 0 && !options.includeComplete) continue

		const selected = pending.slice(
			offset,
			limit === Number.POSITIVE_INFINITY ? undefined : offset + limit,
		)
		results.push({
			locale,
			totalMessages,
			translatedMessages,
			pendingMessages: pending.length,
			missing: pending.filter((entry) => entry.reason === 'missing').length,
			incompatible,
			entries: selected,
			remaining: pending.length - selected.length,
		})
	}

	return results
}

function wrap(text: string, width: number): string[] {
	const lines: string[] = []
	let current = ''
	for (const word of text.split(/\s+/)) {
		if (current && current.length + word.length + 1 > width) {
			lines.push(current)
			current = word
		} else {
			current = current ? `${current} ${word}` : word
		}
	}
	if (current) lines.push(current)
	return lines.length > 0 ? lines : ['']
}

function colorPercent(percent: number): string {
	if (percent >= 90) return chalk.green.bold(`${percent}%`)
	if (percent >= 75) return chalk.yellow.bold(`${percent}%`)
	if (percent >= 50) return chalk.hex('#FFA500').bold(`${percent}%`)
	return chalk.red.bold(`${percent}%`)
}

function colorCount(count: number): string {
	if (count >= 1000) return chalk.red.bold(`${count}`)
	if (count >= 200) return chalk.yellow.bold(`${count}`)
	return chalk.white(`${count}`)
}

function printReport(
	gaps: LocaleGaps[],
	options: CollectGapsOptions,
	hintCoverage?: HintCoverage[],
) {
	console.log()
	console.log(chalk.bold.cyan('  ◎ i18n Pending Translations'))
	console.log(chalk.gray(`  ${'─'.repeat(45)}`))
	console.log()

	if (gaps.length === 0) {
		console.log(chalk.green('  No hay traducciones pendientes con estos filtros.'))
		console.log()
		return
	}

	const totalPending = gaps.reduce((total, gaps) => total + gaps.pendingMessages, 0)

	for (const entry of gaps) {
		const percent =
			entry.totalMessages > 0
				? Math.round((entry.translatedMessages / entry.totalMessages) * 100)
				: 100

		console.log(
			`  ${chalk.white.bold(entry.locale)} ${chalk.gray('—')} ${colorCount(entry.pendingMessages)} ${chalk.gray('pendientes')} ${chalk.gray(`de ${entry.totalMessages}`)} ${colorPercent(percent)}`,
		)

		if (entry.incompatible > 0) {
			console.log(`    ${chalk.magenta(`${entry.incompatible} con contrato ICU incompatible`)}`)
		}

		for (const message of entry.entries) {
			const marker = message.reason === 'incompatible' ? chalk.magenta('(!)') : chalk.cyan('▸')
			console.log(
				`    ${marker} ${chalk.gray(message.scope.padEnd(3))} ${chalk.white(message.key)}`,
			)
			const lines = wrap(message.source, 88)
			lines.forEach((line, index) => {
				const open = index === 0 ? '"' : ' '
				const close = index === lines.length - 1 ? '"' : ''
				console.log(chalk.gray(`        ${open}${line}${close}`))
			})

			for (const [code, text] of Object.entries(message.hints ?? {})) {
				const hintLines = wrap(text, 84)
				hintLines.forEach((line, index) => {
					const open = index === 0 ? `~ ${code} «` : ' '
					const close = index === hintLines.length - 1 ? '»' : ''
					console.log(chalk.hex('#8FB8DE')(`        ${open}${line}${close}`))
				})
			}
		}

		if (entry.remaining > 0) {
			const next = (options.offset ?? 0) + entry.entries.length
			console.log(
				`    ${chalk.gray(`… y ${entry.remaining} más (usa --limit y --offset ${next})`)}`,
			)
		}

		console.log()
	}

	for (const line of hintCoverageLines(hintCoverage)) console.log(line)

	console.log(chalk.gray(`  ${'─'.repeat(45)}`))
	console.log(
		`  ${chalk.gray('Idiomas con pendientes')} ${chalk.white.bold(gaps.length)} ${chalk.gray('· Total pendiente')} ${colorCount(totalPending)}`,
	)
	console.log(chalk.gray('  Usa --json o --out para llevarte un lote a un fichero.'))
	console.log()
}

interface HintCoverage {
	locale: string
	translated: number
	total: number
}

/** Coverage messages for each related locale requested with --hint. */
function hintCoverageLines(hintCoverage: HintCoverage[] | undefined): string[] {
	const lines: string[] = []
	for (const hint of hintCoverage ?? []) {
		const percent = hint.total > 0 ? Math.round((hint.translated / hint.total) * 100) : 100
		lines.push(
			`  ${chalk.gray('Sugerencias de')} ${chalk.white.bold(hint.locale)} ${colorPercent(percent)} ${chalk.gray(`(${hint.translated}/${hint.total} claves)`)}`,
		)
		if (percent < 100) {
			lines.push(chalk.gray('    Algunos huecos no tendrán texto de referencia.'))
		}
	}
	return lines
}

function readOptions(args: string[]): Record<string, string | boolean> {
	const options: Record<string, string | boolean> = {}
	for (let index = 0; index < args.length; index++) {
		const arg = args[index]
		if (!arg.startsWith('-')) continue
		const key = arg.startsWith('--') ? arg.slice(2) : arg.slice(1)

		// Accepts --key=value.
		const equals = key.indexOf('=')
		if (equals !== -1) {
			options[key.slice(0, equals)] = key.slice(equals + 1)
			continue
		}

		const next = args[index + 1]
		if (!next || next.startsWith('-')) {
			options[key] = true
		} else {
			options[key] = next
			index++
		}
	}
	return options
}

const USAGE = `Uso: pnpm i18n:gaps [opciones]  (o pnpm scripts i18n-gaps)

Lista las claves sin traducir de cada idioma junto a su texto en inglés.

Opciones:
  -l, --locale <códigos>  Limita a estos idiomas (separados por coma)
      --scope <app|ui>    Mira solo un catálogo: app (app-frontend) o ui
      --offset <n>        Salta las primeras n claves (para lotes)
      --limit <n>         Máximo de claves por idioma (por defecto ${DEFAULT_LIMIT} en texto, sin límite en --json)
      --all               Incluye idiomas sin pendientes
      --hint <códigos>    Idiomas hermanos de los que tomar la traducción de
                          cada hueco (separados por coma), p. ej. --hint ru-RU
      --json              Salida JSON (para rellenar por lotes)
      --out <ruta>        Escribe el JSON en un fichero
      --help              Muestra esta ayuda
`

function parseNumber(value: string | boolean | undefined, name: string): number | undefined {
	if (value === undefined) return undefined
	if (typeof value === 'boolean') throw new Error(`${name} requiere un número`)
	const parsed = Number(value)
	if (!Number.isInteger(parsed) || parsed < 0) throw new Error(`${name} debe ser un entero >= 0`)
	return parsed
}

function outputFileName(rootDir: string, file: string): string {
	return relativePath(rootDir, path.resolve(rootDir, file))
}

function buildJson(gaps: LocaleGaps[]) {
	const locales: Record<string, unknown> = {}
	const hintLocales = new Set<string>()
	for (const entry of gaps) {
		locales[entry.locale] = {
			totalMessages: entry.totalMessages,
			translatedMessages: entry.translatedMessages,
			pendingMessages: entry.pendingMessages,
			missing: entry.missing,
			incompatible: entry.incompatible,
			entries: entry.entries.map((message) => {
				for (const code of Object.keys(message.hints ?? {})) hintLocales.add(code)
				return {
					key: message.key,
					scope: message.scope,
					file: message.file,
					reason: message.reason,
					source: message.source,
					...(message.hints ? { hints: message.hints } : {}),
				}
			}),
			remaining: entry.remaining,
		}
	}
	return {
		sourceLocale: SOURCE_LOCALE,
		...(hintLocales.size > 0 ? { hintLocales: [...hintLocales].sort() } : {}),
		locales,
	}
}

function main() {
	const options = readOptions(process.argv.slice(2))

	if (options.help || options.h) {
		console.log(USAGE)
		return
	}

	const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
	const jsonOutput = options.json === true || typeof options.out === 'string'

	const locales =
		typeof options.locale === 'string'
			? options.locale.split(',')
			: typeof options.l === 'string'
				? options.l.split(',')
				: undefined

	const scope = options.scope
	if (scope !== undefined && scope !== 'app' && scope !== 'ui') {
		throw new Error(`--scope debe ser app o ui (recibido: ${String(scope)})`)
	}

	const limit =
		parseNumber(options.limit, '--limit') ?? (jsonOutput ? Number.POSITIVE_INFINITY : DEFAULT_LIMIT)

	const hintOption = options.hint ?? options.hints
	if (hintOption !== undefined && typeof hintOption !== 'string') {
		throw new Error('--hint requiere uno o varios códigos de idioma (p. ej. --hint ru-RU)')
	}
	const hintLocales =
		typeof hintOption === 'string' ? hintOption.split(',').map((code) => code.trim()) : undefined

	const gaps = collectGaps(rootDir, {
		locales,
		scope: scope as ScopeId | undefined,
		offset: parseNumber(options.offset, '--offset'),
		limit,
		includeComplete: options.all === true,
		hintLocales,
	})

	// Report coverage for related locales that still have missing entries.
	const hintCoverage = hintLocales?.map((locale) => {
		const [info] = collectGaps(rootDir, {
			locales: [locale],
			scope: scope as ScopeId | undefined,
			limit: 0,
			includeComplete: true,
		})
		return {
			locale: info?.locale ?? locale,
			translated: info?.translatedMessages ?? 0,
			total: info?.totalMessages ?? 0,
		}
	})

	const json = buildJson(gaps)

	if (typeof options.out === 'string') {
		const output = path.resolve(rootDir, options.out)
		fs.mkdirSync(path.dirname(output), { recursive: true })
		fs.writeFileSync(output, `${JSON.stringify(json, null, 2)}\n`)
		console.log(`Escrito ${outputFileName(rootDir, options.out)}`)
	}

	if (options.json === true) {
		console.log(JSON.stringify(json, null, 2))
		return
	}

	if (typeof options.out !== 'string') {
		printReport(gaps, options, hintCoverage)
		return
	}

	const hintLines = hintCoverageLines(hintCoverage)
	if (hintLines.length > 0) {
		console.log()
		for (const line of hintLines) console.log(line)
	}
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
	try {
		main()
	} catch (error) {
		console.error(error instanceof Error ? error.message : error)
		process.exit(1)
	}
}
