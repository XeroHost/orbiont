<script setup>
import {
	CheckIcon,
	CopyIcon,
	DropdownIcon,
	FolderOpenIcon,
	HammerIcon,
	LogInIcon,
	UpdatedIcon,
	WrenchIcon,
	XIcon,
} from '@orbiont/assets'
import { productName, supportUrl } from '@orbiont/branding'
import {
	Button,
	ButtonLink,
	Collapsible,
	commonMessages,
	defineMessages,
	IconButton,
	injectNotificationManager,
	useVIntl,
} from '@orbiont/ui'
import { computed, ref } from 'vue'

import { ChatIcon } from '@/assets/icons'
import ModalWrapper from '@/components/ui/modal/ModalWrapper.vue'
import { handleSevereError } from '@/composables/use-error.js'
import { login as login_flow, set_default_user } from '@/helpers/auth.js'
import { install_existing_instance } from '@/helpers/install'
import { cancel_directory_change } from '@/helpers/settings.ts'
import { showAppDbBackupsFolder } from '@/helpers/utils.js'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	openBackupsFolder: {
		id: 'app.error.state-init.open-backups-folder',
		defaultMessage: 'Open backups folder',
	},
	genericTitle: {
		id: 'app.error.title.generic',
		defaultMessage: 'An error occurred',
	},
	signInFailedTitle: {
		id: 'app.error.title.sign-in-failed',
		defaultMessage: 'Unable to sign in to Minecraft',
	},
	changeDirFailedTitle: {
		id: 'app.error.title.change-dir-failed',
		defaultMessage: 'Could not change app directory',
	},
	noLoaderSelectedTitle: {
		id: 'app.error.title.no-loader-selected',
		defaultMessage: 'No loader selected',
	},
	stateInitTitle: {
		id: 'app.error.title.state-init',
		defaultMessage: 'Error initializing {product}',
	},
	noErrorMessage: {
		id: 'app.error.no-error-message',
		defaultMessage: 'No error message.',
	},
	networkHeading: {
		id: 'app.error.network.heading',
		defaultMessage: 'Network issues',
	},
	networkBodyStart: {
		id: 'app.error.network.body-start',
		defaultMessage: 'It looks like there were issues with',
	},
	networkBodyEnd: {
		id: 'app.error.network.body-end',
		defaultMessage:
			"connecting to Microsoft's servers. This is often the result of a poor connection, so we recommend trying again to see if it works. If issues continue to persist, follow the steps in",
	},
	supportArticleLink: {
		id: 'app.error.support-article-link',
		defaultMessage: 'our support article',
	},
	troubleshootEnd: {
		id: 'app.error.network.troubleshoot-end',
		defaultMessage: 'to troubleshoot.',
	},
	hostsBodyEnd: {
		id: 'app.error.hosts.body-end',
		defaultMessage:
			'tried to connect to Microsoft / Xbox / Minecraft services, but the remote server rejected the connection. This may indicate that these services are blocked by the hosts file. Please visit',
	},
	hostsFixEnd: {
		id: 'app.error.hosts.fix-end',
		defaultMessage: 'for steps on how to fix the issue.',
	},
	tryAnotherAccountHeading: {
		id: 'app.error.try-another-account.heading',
		defaultMessage: 'Try another Microsoft account',
	},
	tryAnotherAccountBody: {
		id: 'app.error.try-another-account.body',
		defaultMessage:
			"Double check you've signed in with the right account. You may own Minecraft on a different Microsoft account.",
	},
	tryAnotherAccountButton: {
		id: 'app.error.try-another-account.button',
		defaultMessage: 'Try another account',
	},
	gamePassHeading: {
		id: 'app.error.game-pass.heading',
		defaultMessage: 'Using PC Game Pass, coming from Bedrock, or just bought the game?',
	},
	gamePassBodyStart: {
		id: 'app.error.game-pass.body-start',
		defaultMessage: 'Try signing in with the',
	},
	gamePassLauncherLink: {
		id: 'app.error.game-pass.launcher-link',
		defaultMessage: 'official Minecraft Launcher',
	},
	gamePassBodyEnd: {
		id: 'app.error.game-pass.body-end',
		defaultMessage: "first. Once you're done, come back here and sign in!",
	},
	tryAgainButton: {
		id: 'app.error.try-again.button',
		defaultMessage: 'Try signing in again',
	},
	readOnlyHeading: {
		id: 'app.error.read-only.heading',
		defaultMessage: 'Change directory permissions',
	},
	readOnlyBodyStart: {
		id: 'app.error.read-only.body-start',
		defaultMessage: 'It looks like',
	},
	readOnlyBodyEnd: {
		id: 'app.error.read-only.body-end',
		defaultMessage:
			'is unable to write to the directory you selected. Please adjust the permissions of the directory and try again or cancel the directory change.',
	},
	notEnoughSpaceHeading: {
		id: 'app.error.not-enough-space.heading',
		defaultMessage: 'Not enough space',
	},
	notEnoughSpaceBody: {
		id: 'app.error.not-enough-space.body',
		defaultMessage:
			'It looks like there is not enough space on the disk containing the directory you selected. Please free up some space and try again or cancel the directory change.',
	},
	migrateBodyEnd: {
		id: 'app.error.migrate.body-end',
		defaultMessage:
			'is unable to migrate to the new directory you selected. Please contact support for help or cancel the directory change.',
	},
	retryDirButton: {
		id: 'app.error.retry-dir.button',
		defaultMessage: 'Retry directory change',
	},
	cancelDirButton: {
		id: 'app.error.cancel-dir.button',
		defaultMessage: 'Cancel directory change',
	},
	stateInitBodyEnd: {
		id: 'app.error.state-init.body-end',
		defaultMessage:
			'failed to load correctly. This may be because of a corrupted file, or because the app is missing crucial files.',
	},
	stateInitFixIntro: {
		id: 'app.error.state-init.fix-intro',
		defaultMessage: 'You may be able to fix it through one of the following ways:',
	},
	stateInitFixOnline: {
		id: 'app.error.state-init.fix-online',
		defaultMessage: 'Ensuring you are connected to the internet, then try restarting the app.',
	},
	stateInitFixRedownload: {
		id: 'app.error.state-init.fix-redownload',
		defaultMessage: 'Redownloading the app.',
	},
	noLoaderBodyEnd: {
		id: 'app.error.no-loader.body-end',
		defaultMessage: 'failed to find the loader version for this instance.',
	},
	noLoaderFix: {
		id: 'app.error.no-loader.fix',
		defaultMessage:
			'To resolve this, you need to repair the instance. Click the button below to do so.',
	},
	repairInstanceButton: {
		id: 'app.error.repair-instance.button',
		defaultMessage: 'Repair instance',
	},
	helpStart: {
		id: 'app.error.help.start',
		defaultMessage: 'If nothing is working and you need help, visit',
	},
	helpSupportLink: {
		id: 'app.error.help.support-link',
		defaultMessage: 'our support page',
	},
	helpEnd: {
		id: 'app.error.help.end',
		defaultMessage:
			'and we will be more than happy to assist! Make sure to include the following debug information:',
	},
	getSupportButton: {
		id: 'app.error.get-support.button',
		defaultMessage: 'Get support',
	},
	debugInfoHeading: {
		id: 'app.error.debug-info.heading',
		defaultMessage: 'Debug information',
	},
	copyDebugInfo: {
		id: 'app.error.copy-debug-info',
		defaultMessage: 'Copy debug info',
	},
})

const errorModal = ref()
const error = ref()
const closable = ref(true)
const errorCollapsed = ref(false)

const title = ref(formatMessage(messages.genericTitle))
const errorType = ref('unknown')
const supportLink = ref(supportUrl)
const metadata = ref({})

defineExpose({
	async show(errorVal, context, canClose = true, source = null) {
		console.log(errorVal, context, canClose, source)
		closable.value = canClose

		if (errorVal.message && errorVal.message.includes('Minecraft authentication error:')) {
			title.value = formatMessage(messages.signInFailedTitle)
			errorType.value = 'minecraft_auth'
			supportLink.value = supportUrl

			if (
				errorVal.message.includes('existing connection was forcibly closed') ||
				errorVal.message.includes('error sending request for url')
			) {
				metadata.value.network = true
			}
			if (errorVal.message.includes('because the target machine actively refused it')) {
				metadata.value.hostsFile = true
			}
		} else if (errorVal.message && errorVal.message.includes('Move directory error:')) {
			title.value = formatMessage(messages.changeDirFailedTitle)
			errorType.value = 'directory_move'
			supportLink.value = supportUrl

			if (errorVal.message.includes('directory is not writable')) {
				metadata.value.readOnly = true
			}

			if (errorVal.message.includes('Not enough space')) {
				metadata.value.notEnoughSpace = true
			}
		} else if (errorVal.message && errorVal.message.includes('No loader version selected for')) {
			title.value = formatMessage(messages.noLoaderSelectedTitle)
			errorType.value = 'no_loader_version'
			supportLink.value = supportUrl
			metadata.value.instanceId = context.instanceId
		} else if (source === 'state_init') {
			title.value = formatMessage(messages.stateInitTitle, { product: productName })
			errorType.value = 'state_init'
			supportLink.value = supportUrl
		} else {
			title.value = formatMessage(messages.genericTitle)
			errorType.value = 'unknown'
			supportLink.value = supportUrl
			metadata.value = {}
		}

		error.value = errorVal
		errorModal.value.show()
	},
})

const loadingMinecraft = ref(false)
async function loginMinecraft() {
	try {
		loadingMinecraft.value = true
		const loggedIn = await login_flow()

		if (loggedIn) {
			await set_default_user(loggedIn.profile.id).catch(handleError)
		}

		loadingMinecraft.value = false
		errorModal.value.hide()
	} catch (err) {
		loadingMinecraft.value = false
		handleSevereError(err)
	}
}

async function cancelDirectoryChange() {
	try {
		await cancel_directory_change()
		window.location.reload()
	} catch (err) {
		handleError(err)
	}
}

function retryDirectoryChange() {
	window.location.reload()
}

async function openDbBackupsFolder() {
	await showAppDbBackupsFolder().catch(handleError)
}

const loadingRepair = ref(false)
async function repairInstance() {
	loadingRepair.value = true
	try {
		await install_existing_instance(metadata.value.instanceId, false)
		errorModal.value.hide()
	} catch (err) {
		handleSevereError(err)
	}
	loadingRepair.value = false
}

const hasDebugInfo = computed(
	() =>
		errorType.value === 'directory_move' ||
		errorType.value === 'minecraft_auth' ||
		errorType.value === 'state_init' ||
		errorType.value === 'no_loader_version',
)

const debugInfo = computed(
	() => error.value.message ?? error.value ?? formatMessage(messages.noErrorMessage),
)

const copied = ref(false)

async function copyToClipboard(text) {
	await navigator.clipboard.writeText(text)
	copied.value = true
	setTimeout(() => {
		copied.value = false
	}, 3000)
}
</script>

<template>
	<ModalWrapper ref="errorModal" :header="title" :closable="closable">
		<div class="modal-body max-w-[550px]">
			<div class="markdown-body">
				<template v-if="errorType === 'minecraft_auth'">
					<template v-if="metadata.network">
						<h3>{{ formatMessage(messages.networkHeading) }}</h3>
						<p>
							{{ formatMessage(messages.networkBodyStart) }} {{ productName }}
							{{ formatMessage(messages.networkBodyEnd) }}
							<a :href="supportUrl"> {{ formatMessage(messages.supportArticleLink) }} </a>
							{{ formatMessage(messages.troubleshootEnd) }}
						</p>
					</template>
					<template v-else-if="metadata.hostsFile">
						<h3>{{ formatMessage(messages.networkHeading) }}</h3>
						<p>
							{{ productName }}
							{{ formatMessage(messages.hostsBodyEnd) }}
							<a :href="supportUrl"> {{ formatMessage(messages.supportArticleLink) }} </a>
							{{ formatMessage(messages.hostsFixEnd) }}
						</p>
					</template>
					<template v-else>
						<h3>{{ formatMessage(messages.tryAnotherAccountHeading) }}</h3>
						<p>
							{{ formatMessage(messages.tryAnotherAccountBody) }}
						</p>
						<div class="cta-button">
							<button class="btn btn-primary" :disabled="loadingMinecraft" @click="loginMinecraft">
								<LogInIcon /> {{ formatMessage(messages.tryAnotherAccountButton) }}
							</button>
						</div>
						<h3>{{ formatMessage(messages.gamePassHeading) }}</h3>
						<p>
							{{ formatMessage(messages.gamePassBodyStart) }}
							<a href="https://www.minecraft.net/en-us/download">{{
								formatMessage(messages.gamePassLauncherLink)
							}}</a>
							{{ formatMessage(messages.gamePassBodyEnd) }}
						</p>
					</template>
					<div class="cta-button">
						<button class="btn btn-primary" :disabled="loadingMinecraft" @click="loginMinecraft">
							<LogInIcon /> {{ formatMessage(messages.tryAgainButton) }}
						</button>
					</div>
				</template>
				<template v-if="errorType === 'directory_move'">
					<template v-if="metadata.readOnly">
						<h3>{{ formatMessage(messages.readOnlyHeading) }}</h3>
						<p>
							{{ formatMessage(messages.readOnlyBodyStart) }} {{ productName }}
							{{ formatMessage(messages.readOnlyBodyEnd) }}
						</p>
					</template>
					<template v-else-if="metadata.notEnoughSpace">
						<h3>{{ formatMessage(messages.notEnoughSpaceHeading) }}</h3>
						<p>
							{{ formatMessage(messages.notEnoughSpaceBody) }}
						</p>
					</template>
					<template v-else>
						<p>
							{{ productName }}
							{{ formatMessage(messages.migrateBodyEnd) }}
						</p>
					</template>

					<div class="cta-button">
						<button class="btn" @click="retryDirectoryChange">
							<UpdatedIcon /> {{ formatMessage(messages.retryDirButton) }}
						</button>
						<button class="btn btn-danger" @click="cancelDirectoryChange">
							<XIcon /> {{ formatMessage(messages.cancelDirButton) }}
						</button>
					</div>
				</template>
				<template v-else-if="errorType === 'state_init'">
					<p>
						{{ productName }}
						{{ formatMessage(messages.stateInitBodyEnd) }}
					</p>
					<p>{{ formatMessage(messages.stateInitFixIntro) }}</p>
					<ul>
						<li>{{ formatMessage(messages.stateInitFixOnline) }}</li>
						<li>{{ formatMessage(messages.stateInitFixRedownload) }}</li>
					</ul>
				</template>
				<template v-else-if="errorType === 'no_loader_version'">
					<p>{{ productName }} {{ formatMessage(messages.noLoaderBodyEnd) }}</p>
					<p>{{ formatMessage(messages.noLoaderFix) }}</p>
					<div class="cta-button">
						<button class="btn btn-primary" :disabled="loadingRepair" @click="repairInstance">
							<HammerIcon /> {{ formatMessage(messages.repairInstanceButton) }}
						</button>
					</div>
				</template>
				<template v-else>
					{{ debugInfo }}
				</template>
				<template v-if="hasDebugInfo">
					<div class="w-full h-[1px] bg-surface-5 mb-3"></div>
					<p>
						{{ formatMessage(messages.helpStart) }}
						<a :href="supportLink">{{ formatMessage(messages.helpSupportLink) }}</a>
						{{ formatMessage(messages.helpEnd) }}
					</p>
				</template>
			</div>
			<div class="flex items-center gap-2">
				<ButtonLink :href="supportLink" @click="errorModal.hide()"
					><ChatIcon /> {{ formatMessage(messages.getSupportButton) }}</ButtonLink
				>
				<Button v-if="closable" @click="errorModal.hide()"
					><XIcon /> {{ formatMessage(commonMessages.closeButton) }}</Button
				>
			</div>
			<template v-if="hasDebugInfo">
				<div class="flex flex-col gap-2">
					<div class="w-full h-[1px] bg-surface-5"></div>

					<div class="overflow-clip">
						<button
							class="flex items-center justify-between w-full bg-transparent border-0 py-4 cursor-pointer"
							@click="errorCollapsed = !errorCollapsed"
						>
							<span class="flex items-center gap-2 text-contrast font-extrabold m-0">
								<WrenchIcon class="h-4 w-4" />
								{{ formatMessage(messages.debugInfoHeading) }}
							</span>
							<DropdownIcon
								class="h-5 w-5 text-secondary transition-transform"
								:class="{ 'rotate-180': !errorCollapsed }"
							/>
						</button>
						<Collapsible :collapsed="errorCollapsed">
							<div
								class="p-3 bg-surface-2 rounded-2xl text-xs grid grid-cols-[1fr_auto] max-w-full items-start"
							>
								<div
									class="m-0 p-0 rounded-none bg-transparent text-sm font-mono break-words overflow-auto"
								>
									{{ debugInfo }}
									<button class="btn" @click="openDbBackupsFolder">
										<FolderOpenIcon aria-hidden="true" />
										{{ formatMessage(messages.openBackupsFolder) }}
									</button>
								</div>
								<IconButton
									v-tooltip="formatMessage(messages.copyDebugInfo)"
									:label="formatMessage(messages.copyDebugInfo)"
									:disabled="copied"
									@click="copyToClipboard(debugInfo)"
								>
									<template v-if="copied"> <CheckIcon class="text-green" /> </template>
									<template v-else> <CopyIcon /> </template>
								</IconButton>
							</div>
						</Collapsible>
					</div>
				</div>
			</template>
		</div>
	</ModalWrapper>
</template>

<style>
.light-mode {
	--color-orange-bg: rgba(255, 163, 71, 0.2);
}

.dark-mode,
.oled-mode {
	--color-orange-bg: rgba(224, 131, 37, 0.2);
}
</style>

<style scoped lang="scss">
.cta-button {
	display: flex;
	align-items: center;
	justify-content: center;
	padding: 0.5rem;
	gap: 0.5rem;
}

.warning-banner {
	display: flex;
	flex-direction: column;
	gap: 0.5rem;
	padding: var(--gap-lg);
	background-color: var(--color-orange-bg);
	border: 2px solid var(--color-orange);
	border-radius: var(--radius-md);
	margin-bottom: 1rem;
}

.warning-banner__title {
	display: flex;
	align-items: center;
	gap: 0.5rem;
	font-weight: 700;

	svg {
		color: var(--color-orange);
		height: 1.5rem;
		width: 1.5rem;
	}
}

.modal-body {
	display: flex;
	flex-direction: column;
	gap: var(--gap-md);
}

.markdown-body {
	overflow: auto;
}
</style>
