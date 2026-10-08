import cssText from '@orbiont/assets/styles/ace.css?raw'
import ace from 'ace-builds'

import { defineAceModule } from './ace-define'

// Editing/search do not need background lint workers or dynamic worker URLs.
ace.config.setDefaultValue('session', 'useWorker', false)

defineAceModule(
	'ace/theme/modrinth',
	['require', 'exports', 'module', 'ace/lib/dom'],
	function (require, exports) {
		exports.isDark = false
		exports.cssClass = 'ace-modrinth'
		exports.cssText = cssText

		const dom = require('ace/lib/dom') as {
			importCssString: (text: string, className: string, insertFirst: boolean) => void
		}
		dom.importCssString(cssText, 'ace-modrinth', false)
	},
)
