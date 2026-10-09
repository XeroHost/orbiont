import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import ts from 'typescript'

for (const [file, name] of [
	['skin-preview-storage', 'SkinPreviewStorage'],
	['head-storage', 'HeadStorage'],
]) {
	test(`${name} removes stale keys and waits for commit; aborted cleanup rejects`, async () => {
		const source = readFileSync(
			new URL(`../src/helpers/storage/${file}.ts`, import.meta.url),
			'utf8',
		)
		const code = ts.transpileModule(source, {
			compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
		}).outputText
		const module = { exports: {} }
		new Function('module', 'exports', code)(module, module.exports)
		const storage = new module.exports[name]()
		const keys = new Set(['current', 'obsolete'])
		let transaction
		storage.db = {
			transaction() {
				const store = {
					openKeyCursor() {
						const request = {}
						const remaining = [...keys]
						const next = () =>
							queueMicrotask(() => {
								const key = remaining.shift()
								const cursor =
									key === undefined
										? null
										: {
												primaryKey: key,
												delete() {
													throw new DOMException('The cursor is a key cursor.', 'InvalidStateError')
												},
												continue: next,
											}
								try {
									request.onsuccess({ target: { result: cursor } })
								} catch (error) {
									transaction.error = error
									transaction.onabort?.()
								}
							})
						next()
						return request
					},
					delete(key) {
						const request = {}
						queueMicrotask(() => {
							keys.delete(key)
							request.onsuccess?.()
						})
						return request
					},
				}
				transaction = { objectStore: () => store }
				return transaction
			},
		}
		let settled = false
		const cleanup = storage.cleanupInvalidKeys(new Set(['current']))
		cleanup.then(
			() => {
				settled = true
			},
			() => {},
		)
		await new Promise((resolve) => setImmediate(resolve))
		assert.equal(transaction.error, undefined, 'key cursors cannot delete values')
		assert.equal(settled, false, 'cleanup must wait for the transaction to commit')
		transaction.oncomplete?.()
		assert.equal(await cleanup, 1)
		assert.deepEqual([...keys], ['current'])
		const aborted = storage.cleanupInvalidKeys(new Set(['current']))
		const rejected = assert.rejects(aborted, /aborted/)
		transaction.error = new Error('transaction aborted')
		transaction.onabort?.()
		await rejected
	})
}
