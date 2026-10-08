<script setup>
import {
	ClipboardCopyIcon,
	ExternalIcon,
	GlobeIcon,
	MailIcon,
	MastodonIcon,
	RedditIcon,
	ShareIcon,
	TwitterIcon,
} from '@orbiont/assets'
import QrcodeVue from 'qrcode.vue'
import { computed, nextTick, ref } from 'vue'

import { ButtonLink, IconButton } from '#ui/components/base/buttons'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { injectNotificationManager } from '#ui/providers'

import { useDebugLogger } from '../../composables/debug-logger'
import { NewModal, Textarea } from '../index'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	copyQrCode: {
		id: 'modal.share.copy-qr-code',
		defaultMessage: 'Copy QR code',
	},
	copyText: {
		id: 'modal.share.copy-text',
		defaultMessage: 'Copy Text',
	},
	copyLink: {
		id: 'modal.share.copy-link',
		defaultMessage: 'Copy Link',
	},
	share: {
		id: 'modal.share.share',
		defaultMessage: 'Share',
	},
	sendEmail: {
		id: 'modal.share.send-email',
		defaultMessage: 'Send as an email',
	},
	openLinkInBrowser: {
		id: 'modal.share.open-link-in-browser',
		defaultMessage: 'Open link in browser',
	},
	tootAboutIt: {
		id: 'modal.share.toot-about-it',
		defaultMessage: 'Toot about it',
	},
	tweetAboutIt: {
		id: 'modal.share.tweet-about-it',
		defaultMessage: 'Tweet about it',
	},
	shareOnReddit: {
		id: 'modal.share.share-on-reddit',
		defaultMessage: 'Share on Reddit',
	},
	openInNewTab: {
		id: 'modal.share.open-in-new-tab',
		defaultMessage: 'Open in new tab',
	},
	linkCopiedTitle: {
		id: 'modal.share.link-copied-title',
		defaultMessage: 'Link copied',
	},
	linkCopiedText: {
		id: 'modal.share.link-copied-text',
		defaultMessage: 'The link has been copied to your clipboard.',
	},
	copyFailedTitle: {
		id: 'modal.share.copy-failed-title',
		defaultMessage: 'Failed to copy text',
	},
})

const debug = useDebugLogger('ShareModal')

const props = defineProps({
	header: {
		type: String,
		default: 'Share',
	},
	shareTitle: {
		type: String,
		default: 'Modrinth',
	},
	shareText: {
		type: String,
		default: null,
	},
	link: {
		type: Boolean,
		default: false,
	},
	openInNewTab: {
		type: Boolean,
		default: true,
	},
	noblur: {
		type: Boolean,
		default: false,
	},
	socialButtons: {
		type: Boolean,
		default: true,
	},
	onHide: {
		type: Function,
		default() {
			return () => {}
		},
	},
})

const shareModal = ref(null)
const { addNotification } = injectNotificationManager()

const qrCode = ref(null)
const qrImage = ref(null)
const content = ref(null)
const url = ref(null)
const canShare = ref(false)
const share = () => {
	navigator.share(
		props.link
			? {
					title: props.shareTitle.toString(),
					text: props.shareText,
					url: url.value,
				}
			: {
					title: props.shareTitle.toString(),
					text: content.value,
				},
	)
}

const show = async (passedContent) => {
	content.value = props.shareText ? `${props.shareText}\n\n${passedContent}` : passedContent
	shareModal.value.show()
	if (props.link) {
		url.value = passedContent
		nextTick(() => {
			debug(qrCode.value)
			fetch(qrCode.value.getElementsByTagName('canvas')[0].toDataURL('image/png'))
				.then((res) => res.blob())
				.then((blob) => {
					debug(blob)
					qrImage.value = blob
				})
		})
	}
	if (navigator.canShare({ title: props.shareTitle.toString(), text: content.value })) {
		canShare.value = true
	}
}

const copyImage = async () => {
	const item = new ClipboardItem({ 'image/png': qrImage.value })
	await navigator.clipboard.write([item])
}

const copyText = async () => {
	try {
		await navigator.clipboard.writeText(url.value ?? content.value)
		addNotification({
			type: 'success',
			title: formatMessage(messages.linkCopiedTitle),
			text: formatMessage(messages.linkCopiedText),
		})
	} catch (error) {
		const message = error instanceof Error ? error.message : String(error)
		addNotification({
			type: 'error',
			title: formatMessage(messages.copyFailedTitle),
			text: message,
		})
	}
}

const sendEmail = computed(
	() =>
		`mailto:user@test.com
    ?subject=${encodeURIComponent(props.shareTitle)}
    &body=${encodeURIComponent(content.value)}`,
)

const targetParameter = computed(() => (props.openInNewTab ? '_blank' : '_self'))

const sendTweet = computed(
	() => `https://twitter.com/intent/tweet?text=${encodeURIComponent(content.value)}`,
)

const sendToot = computed(() => `https://tootpick.org/#text=${encodeURIComponent(content.value)}`)

const postOnReddit = computed(
	() =>
		`https://www.reddit.com/submit?title=${encodeURIComponent(props.shareTitle)}&text=${encodeURIComponent(
			content.value,
		)}`,
)

defineExpose({
	hide: () => shareModal.value?.hide(),
	show,
})
</script>

<template>
	<NewModal ref="shareModal" :header="header" :noblur="noblur" :on-hide="onHide">
		<div class="flex flex-col items-center gap-2">
			<div
				:class="['flex items-center justify-center', link ? 'flex-wrap gap-4' : 'flex-col gap-2']"
			>
				<div v-if="link" class="group relative shrink-0">
					<div ref="qrCode">
						<QrcodeVue :value="url" class="!bg-white rounded-[var(--radius-md)]" margin="3" />
					</div>
					<IconButton
						v-tooltip="formatMessage(messages.copyQrCode)"
						type="quiet"
						:label="formatMessage(messages.copyQrCode)"
						class="absolute top-0 right-0 m-2"
						@click="copyImage"
					>
						<ClipboardCopyIcon class="h-5 w-5" aria-hidden="true" />
					</IconButton>
				</div>
				<Textarea v-else v-model="content" resize="vertical" wrapper-class="h-full w-[30rem]">
					<template #right>
						<IconButton
							v-tooltip="formatMessage(messages.copyText)"
							type="quiet"
							:label="formatMessage(messages.copyText)"
							native-type="button"
							class="absolute top-0 right-0 m-2"
							@click="copyText"
						>
							<ClipboardCopyIcon class="h-5 w-5" aria-hidden="true" />
						</IconButton>
					</template>
				</Textarea>
				<div
					v-if="link || socialButtons"
					:class="['flex flex-col justify-center gap-2', link ? 'w-64 max-w-full' : 'flex-grow']"
				>
					<button
						v-if="link"
						v-tooltip="formatMessage(messages.copyLink)"
						type="button"
						class="flex h-10 w-full cursor-pointer items-center justify-between gap-2 rounded-xl border-none bg-button-bg px-3 pr-1.5 text-primary transition-all hover:bg-button-bg-hover hover:brightness-125 active:scale-95"
						@click="copyText"
					>
						<span class="min-w-0 cursor-pointer truncate text-left font-semibold text-primary">
							{{ url }}
						</span>
						<div class="grid h-10 w-10 place-content-center">
							<ClipboardCopyIcon class="h-5 w-5" aria-hidden="true" />
						</div>
					</button>
					<ButtonLink
						v-if="link"
						:href="url"
						target="_blank"
						rel="noopener noreferrer"
						:aria-label="formatMessage(messages.openInNewTab)"
						class="w-full"
					>
						{{ formatMessage(messages.openInNewTab) }}
						<ExternalIcon aria-hidden="true" />
					</ButtonLink>
					<div v-if="socialButtons" class="flex flex-row gap-1">
						<IconButton
							v-if="canShare"
							v-tooltip="formatMessage(messages.share)"
							:label="formatMessage(messages.share)"
							@click="share"
						>
							<ShareIcon aria-hidden="true" />
						</IconButton>
						<ButtonLink
							v-tooltip="formatMessage(messages.sendEmail)"
							:href="sendEmail"
							:target="targetParameter"
							class="!w-9 !px-0 !rounded-full"
						>
							<MailIcon aria-hidden="true" />
						</ButtonLink>
						<ButtonLink
							v-if="link"
							v-tooltip="formatMessage(messages.openLinkInBrowser)"
							:target="targetParameter"
							:href="url"
							class="!w-9 !px-0 !rounded-full"
						>
							<GlobeIcon aria-hidden="true" />
						</ButtonLink>
						<ButtonLink
							v-tooltip="formatMessage(messages.tootAboutIt)"
							:target="targetParameter"
							:href="sendToot"
							class="!w-9 !px-0 !rounded-full"
						>
							<MastodonIcon aria-hidden="true" />
						</ButtonLink>
						<ButtonLink
							v-tooltip="formatMessage(messages.tweetAboutIt)"
							:target="targetParameter"
							:href="sendTweet"
							class="!w-9 !px-0 !rounded-full"
						>
							<TwitterIcon aria-hidden="true" />
						</ButtonLink>
						<ButtonLink
							v-tooltip="formatMessage(messages.shareOnReddit)"
							:target="targetParameter"
							:href="postOnReddit"
							class="!w-9 !px-0 !rounded-full"
						>
							<RedditIcon aria-hidden="true" />
						</ButtonLink>
					</div>
				</div>
			</div>
		</div>
	</NewModal>
</template>
