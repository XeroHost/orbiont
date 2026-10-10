// Only the launcher WebView, never signin, ads or embedded pages.
;(() => {
	if (window.top && window !== window.top) return
	const origin = location.origin
	if (!(
		origin === 'tauri://localhost' ||
		origin === 'http://tauri.localhost' ||
		origin === 'https://tauri.localhost' ||
		/^https?:\/\/localhost:\d+$/.test(origin)
	))
		return
	let count = 0,
		start = Date.now(),
		sending = false
	function describe(value) {
		try {
			if (typeof value === 'string') return value
			if (value === null || ['number', 'boolean', 'undefined'].includes(typeof value))
				return String(value)
			// Avoid inspecting application objects, DOM, credential payloads or custom toJSON/getters.
			return '[non-text payload omitted]'
		} catch {
			return '[unserializable payload omitted]'
		}
	}
	function send(level, values) {
		if (sending) return
		const now = Date.now()
		if (now - start >= 10000) {
			start = now
			count = 0
		}
		if (count >= 20) return
		count++
		// Drop the whole payload before any cut can hide a suffix needed by native
		// exact-value redaction. Count UTF-8 bytes without allocating encoded input.
		const parts = values.slice(0, 8).map(describe)
		let message = parts.some((part) => part.length > 2048)
			? '[oversized payload omitted]'
			: parts.join(' ')
		let bytes = 0
		for (const character of message) {
			const point = character.codePointAt(0)
			bytes += point <= 0x7f ? 1 : point <= 0x7ff ? 2 : point <= 0xffff ? 3 : 4
			if (bytes > 2048) {
				message = '[oversized payload omitted]'
				break
			}
		}
		message = message
			.replace(
				/((?:access_token|refresh_token|password|client_secret|authorization|api_key|token)\s*["']?\s*[:=]\s*)(?:"[^"]*"|'[^']*'|[^\s&,}]+)/gi,
				'$1[REDACTED]',
			)
			.replace(/Bearer\s+[^\s"'&,}]+/gi, 'Bearer [REDACTED]')
		sending = true
		try {
			Promise.resolve(
				window.__TAURI_INTERNALS__?.invoke('plugin:logs|webview_log', { level, message }),
			).catch(() => {
				if (window.top && window !== window.top) return
			})
		} finally {
			sending = false
		}
	}
	for (const level of ['warn', 'error']) {
		const original = console[level].bind(console)
		console[level] = (...values) => {
			original(...values)
			send(level, values)
		}
	}
	window.addEventListener('error', (event) => send('error', [event.error || event.message]))
	window.addEventListener('unhandledrejection', (event) => send('error', [event.reason]))
})()
