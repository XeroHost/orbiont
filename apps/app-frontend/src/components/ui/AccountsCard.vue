<template>
	<FloatingMenu placement="bottom-end" @open="refreshValues">
		<button
			type="button"
			class="flex max-w-56 items-center gap-2 rounded-xl border border-solid border-surface-5 bg-button-bg py-1 pr-2.5 text-sm font-medium text-contrast transition-[filter] hover:brightness-110"
			:class="selectedAccount ? 'pl-1' : 'pl-3'"
		>
			<template v-if="selectedAccount">
				<Avatar size="24px" :src="avatarUrl" />
				<span class="truncate">{{ selectedAccount.profile.name }}</span>
			</template>
			<template v-else>
				{{ formatMessage(hasExtraContent ? messages.gettingStarted : messages.signIn) }}
			</template>
			<DropdownIcon class="size-4 shrink-0" />
		</button>
		<template #popper="{ hide }">
			<div class="flex w-72 flex-col gap-2 p-1">
				<template v-if="accounts.length === 0">
					<span class="px-1 text-sm text-secondary">{{ formatMessage(messages.notSignedIn) }}</span>
					<Button
						type="colored"
						color="brand"
						:disabled="loginDisabled"
						@click="loginFromMenu(hide)"
					>
						<LogInIcon v-if="!loginDisabled" />
						<SpinnerIcon v-else class="animate-spin" />
						{{ formatMessage(messages.signInToMinecraft) }}
					</Button>
				</template>
				<template v-else>
					<span class="px-1 text-xs font-medium uppercase text-secondary">
						{{ formatMessage(messages.minecraftAccounts) }}
					</span>
					<div
						v-for="account in accounts"
						:key="account.profile.id"
						class="flex items-center gap-1"
					>
						<button
							class="button-base flex min-w-0 flex-shrink flex-grow cursor-pointer items-center gap-2 overflow-clip rounded-xl border-0 bg-transparent p-2"
							@click="setAccount(account)"
						>
							<RadioButtonCheckedIcon
								v-if="isSelected(account)"
								class="h-5 w-5 shrink-0 text-contrast"
							/>
							<RadioButtonIcon v-else class="h-5 w-5 shrink-0 text-secondary" />
							<Avatar :src="getAccountAvatarUrl(account)" size="24px" />
							<p
								class="m-0 min-w-0 truncate"
								:class="isSelected(account) ? 'font-semibold text-contrast' : 'text-primary'"
							>
								{{ account.profile.name }}
							</p>
						</button>
						<IconButton
							v-tooltip="formatMessage(messages.removeAccount)"
							type="quiet"
							color="red"
							:label="formatMessage(messages.removeAccount)"
							class="!bg-button-bg !text-primary ![box-shadow:var(--shadow-button)] hover:!bg-red focus-visible:!bg-red hover:!text-[var(--color-accent-contrast)] focus-visible:!text-[var(--color-accent-contrast)]"
							@click="logout(account.profile.id)"
						>
							<TrashIcon />
						</IconButton>
					</div>
					<Button
						class="w-full !bg-button-bg !text-primary ![box-shadow:var(--shadow-button)]"
						:disabled="loginDisabled"
						@click="loginFromMenu(hide)"
					>
						<PlusIcon />
						{{ formatMessage(messages.addAccount) }}
					</Button>
				</template>
				<!-- Getting-started steps, while any are left (see OnboardingChecklist). -->
				<slot name="extra" :hide="hide" />
			</div>
		</template>
	</FloatingMenu>
</template>

<script setup lang="ts">
import {
	DropdownIcon,
	LogInIcon,
	PlusIcon,
	RadioButtonCheckedIcon,
	RadioButtonIcon,
	SpinnerIcon,
	TrashIcon,
} from '@orbiont/assets'
import {
	Avatar,
	Button,
	defineMessages,
	FloatingMenu,
	IconButton,
	injectNotificationManager,
	useVIntl,
} from '@orbiont/ui'
import type { Ref } from 'vue'
import { computed, onUnmounted, ref, useSlots } from 'vue'

import { useAppEvent } from '@/composables/use-app-event'
import { handleSevereError } from '@/composables/use-error.js'
import {
	get_default_user,
	login as login_flow,
	remove_user,
	set_default_user,
	users,
} from '@/helpers/auth'
import { getPlayerHeadUrl } from '@/helpers/rendering/player-head'
import type { Skin } from '@/helpers/skins'
import { get_available_skins } from '@/helpers/skins'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const slots = useSlots()
// The parent only fills the extra slot while getting-started steps remain.
const hasExtraContent = computed(() => !!slots.extra)

const emit = defineEmits<{
	change: []
}>()

type MinecraftCredential = {
	profile: {
		id: string
		name: string
	}
}

const STEVE_HEAD_URL = 'https://mc-heads.net/avatar/MHF_Steve/128'

const accounts: Ref<MinecraftCredential[]> = ref([])
const loginDisabled = ref(false)
const defaultUser = ref<string | undefined>()
const equippedSkin = ref<Skin | null>(null)
const equippedHeadUrl = ref<string>()
let headRequest = 0

async function updateHeadUrl(skin: Skin | null) {
	const request = ++headRequest
	if (equippedHeadUrl.value) URL.revokeObjectURL(equippedHeadUrl.value)
	equippedHeadUrl.value = undefined
	if (!skin) return
	const url = await getPlayerHeadUrl(skin)
	if (request !== headRequest) URL.revokeObjectURL(url)
	else equippedHeadUrl.value = url
}

onUnmounted(() => {
	headRequest++
	if (equippedHeadUrl.value) URL.revokeObjectURL(equippedHeadUrl.value)
})

async function refreshValues() {
	defaultUser.value = await get_default_user().catch(handleError)
	const userList = await users().catch(handleError)
	accounts.value = Array.isArray(userList) ? [...userList] : []
	accounts.value.sort((a, b) => (a.profile?.name ?? '').localeCompare(b.profile?.name ?? ''))

	try {
		const skins = await get_available_skins()
		equippedSkin.value = skins.find((skin) => skin.is_equipped) ?? null

		await updateHeadUrl(equippedSkin.value)
	} catch {
		equippedSkin.value = null
		void updateHeadUrl(null)
	}
}

async function setEquippedSkin(skin: Skin) {
	equippedSkin.value = skin

	try {
		await updateHeadUrl(skin)
	} catch (error) {
		console.warn('Failed to get head render for equipped skin:', error)
	}
}

function setLoginDisabled(value: boolean) {
	loginDisabled.value = value
}

defineExpose({
	refreshValues,
	setEquippedSkin,
	setLoginDisabled,
	login,
	loginDisabled,
})

await refreshValues()

const selectedAccount = computed(() =>
	accounts.value.find((account) => account.profile.id === defaultUser.value),
)

const avatarUrl = computed(() => {
	if (equippedSkin.value?.texture_key) {
		const cachedUrl = equippedHeadUrl.value
		if (cachedUrl) {
			return cachedUrl
		}
		return `https://mc-heads.net/avatar/${equippedSkin.value.texture_key}/128`
	}
	if (selectedAccount.value?.profile?.id) {
		return `https://mc-heads.net/avatar/${selectedAccount.value.profile.id}/128`
	}
	return STEVE_HEAD_URL
})

function getAccountAvatarUrl(account: MinecraftCredential) {
	if (
		account.profile.id === selectedAccount.value?.profile?.id &&
		equippedSkin.value?.texture_key
	) {
		const cachedUrl = equippedHeadUrl.value
		if (cachedUrl) {
			return cachedUrl
		}
	}
	return `https://mc-heads.net/avatar/${account.profile.id}/128`
}

function isSelected(account: MinecraftCredential) {
	return selectedAccount.value?.profile.id === account.profile.id
}

function loginFromMenu(hide: () => void) {
	hide()
	void login()
}

async function setAccount(account: MinecraftCredential) {
	defaultUser.value = account.profile.id
	await set_default_user(account.profile.id).catch(handleError)
	await refreshValues()
	emit('change')
}

async function login() {
	loginDisabled.value = true
	const loggedIn = await login_flow().catch(handleSevereError)

	if (loggedIn) {
		await setAccount(loggedIn)
	}

	loginDisabled.value = false
}

async function logout(id: string) {
	await remove_user(id).catch(handleError)
	await refreshValues()
	if (!selectedAccount.value && accounts.value.length > 0) {
		await setAccount(accounts.value[0])
	} else {
		emit('change')
	}
}

useAppEvent('process', async (e) => {
	if (e.event === 'launched') {
		await refreshValues()
	}
})

const messages = defineMessages({
	notSignedIn: {
		id: 'minecraft-account.not-signed-in',
		defaultMessage: 'Not signed in',
	},
	addAccount: {
		id: 'minecraft-account.add-account',
		defaultMessage: 'Add account',
	},
	removeAccount: {
		id: 'minecraft-account.remove-account',
		defaultMessage: 'Remove account',
	},
	signInToMinecraft: {
		id: 'minecraft-account.sign-in',
		defaultMessage: 'Sign in to Minecraft',
	},
	signIn: {
		id: 'minecraft-account.sign-in-short',
		defaultMessage: 'Sign in',
	},
	gettingStarted: {
		id: 'minecraft-account.getting-started',
		defaultMessage: 'Getting started',
	},
	minecraftAccounts: {
		id: 'minecraft-account.accounts',
		defaultMessage: 'Minecraft accounts',
	},
})
</script>
