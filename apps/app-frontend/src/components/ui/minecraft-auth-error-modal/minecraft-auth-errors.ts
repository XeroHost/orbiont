import { defineMessage, type MessageDescriptor } from '@orbiont/ui'

export interface MinecraftAuthError {
	errorCode?: string
	errorMatchers?: string[]
	matches?: (message: string) => boolean
	whatHappened: MessageDescriptor
	stepsToFix: MessageDescriptor[]
}

const tryProductAgain = defineMessage({
	id: 'app.auth-error.step.try-product-again',
	defaultMessage: 'Try signing in to {product} again',
})

const onceFinishedTryAgain = defineMessage({
	id: 'app.auth-error.step.once-finished-try-again',
	defaultMessage: 'Once finished, try signing in again',
})

const signInOfficialLauncher = defineMessage({
	id: 'app.auth-error.step.sign-in-official-launcher',
	defaultMessage:
		'Sign in with the <a href="https://www.minecraft.net/en-us/download">official Minecraft Launcher</a>',
})

const visitMinecraftLogin = defineMessage({
	id: 'app.auth-error.step.visit-minecraft-login',
	defaultMessage:
		'Visit <a href="https://www.minecraft.net/en-us/login">Minecraft Login</a> and sign in',
})

const visitXboxSignIn = defineMessage({
	id: 'app.auth-error.step.visit-xbox-sign-in',
	defaultMessage: 'Visit <a href="https://www.xbox.com">Xbox</a> and sign in',
})

export const minecraftAuthErrors: MinecraftAuthError[] = [
	{
		errorMatchers: ['Failed to deserialize response to JSON during step RefreshOAuthToken:'],
		whatHappened: defineMessage({
			id: 'app.auth-error.token-expired.what',
			defaultMessage:
				'Your saved Microsoft sign-in token has expired or was revoked, so {product} cannot refresh your Minecraft session.',
		}),
		stepsToFix: [
			defineMessage({
				id: 'app.auth-error.token-expired.step-sign-out',
				defaultMessage: 'Sign out of the affected Minecraft account in {product}',
			}),
			defineMessage({
				id: 'app.auth-error.token-expired.step-sign-in',
				defaultMessage: 'Sign in to the account again',
			}),
			defineMessage({
				id: 'app.auth-error.token-expired.step-retry',
				defaultMessage: 'Once the new sign-in finishes, try launching Minecraft again',
			}),
		],
	},
	{
		errorMatchers: ['Failed to deserialize response to JSON during step XboxUserAuthenticate:'],
		whatHappened: defineMessage({
			id: 'app.auth-error.xbox-rejected.what',
			defaultMessage:
				'Xbox Live did not accept the Microsoft sign-in. This usually means the Microsoft account has never signed in to Xbox or needs to accept updated terms.',
		}),
		stepsToFix: [
			defineMessage({
				id: 'app.auth-error.xbox-rejected.step-xbox-com',
				defaultMessage:
					'Sign in once at <a href="https://www.xbox.com">Xbox.com</a> with the same Microsoft account',
			}),
			defineMessage({
				id: 'app.auth-error.xbox-rejected.step-terms',
				defaultMessage: 'Accept any terms or complete any profile setup that is shown',
			}),
			tryProductAgain,
		],
	},
	{
		matches: (message) =>
			message.includes('Failed to deserialize response to JSON during step MinecraftToken:') &&
			message.includes('429 Too Many Requests'),
		whatHappened: defineMessage({
			id: 'app.auth-error.rate-limited.what',
			defaultMessage:
				'Microsoft or Minecraft temporarily blocked the sign-in request because there were too many recent attempts.',
		}),
		stepsToFix: [
			defineMessage({
				id: 'app.auth-error.rate-limited.step-wait',
				defaultMessage: 'Wait about an hour before trying again',
			}),
			defineMessage({
				id: 'app.auth-error.rate-limited.step-restart',
				defaultMessage: 'Restart {product} after waiting',
			}),
			defineMessage({
				id: 'app.auth-error.rate-limited.step-retry',
				defaultMessage: 'Try signing in once more',
			}),
			defineMessage({
				id: 'app.auth-error.rate-limited.step-wait-longer',
				defaultMessage:
					'If the same message appears, wait longer before retrying so the temporary limit can clear',
			}),
		],
	},
	{
		matches: (message) =>
			message.includes('Failed to deserialize response to JSON during step MinecraftToken:') &&
			/Status Code: 5\d\d/.test(message),
		whatHappened: defineMessage({
			id: 'app.auth-error.service-error.what',
			defaultMessage:
				"Minecraft's authentication service is returning a server error, so {product} cannot finish signing you in right now.",
		}),
		stepsToFix: [
			defineMessage({
				id: 'app.auth-error.service-error.step-wait',
				defaultMessage: 'Wait a few minutes and try signing in again',
			}),
			defineMessage({
				id: 'app.auth-error.service-error.step-status',
				defaultMessage:
					'Check <a href="https://support.xbox.com/xbox-live-status">Xbox Status</a> for current service issues',
			}),
			defineMessage({
				id: 'app.auth-error.service-error.step-official-launcher',
				defaultMessage:
					'Try signing in with the <a href="https://www.minecraft.net/en-us/download">official Minecraft Launcher</a> to confirm whether Minecraft sign-in is also affected there',
			}),
			defineMessage({
				id: 'app.auth-error.service-error.step-contact',
				defaultMessage:
					'If the service is healthy and this keeps happening, contact support with the debug information below',
			}),
		],
	},
	{
		errorMatchers: ['does not own Minecraft: Java Edition'],
		whatHappened: defineMessage({
			id: 'app.auth-error.no-license.what',
			defaultMessage:
				"This Microsoft account doesn't own Minecraft: Java Edition, and {product} only works with premium accounts.",
		}),
		stepsToFix: [
			defineMessage({
				id: 'app.auth-error.no-license.step-right-account',
				defaultMessage:
					'Make sure you are signing in with the Microsoft account that bought Minecraft (or has an active Game Pass with PC access)',
			}),
			defineMessage({
				id: 'app.auth-error.no-license.step-check-owner',
				defaultMessage:
					'You can check which account owns it at <a href="https://www.minecraft.net/en-us/login">minecraft.net</a>',
			}),
			tryProductAgain,
		],
	},
	{
		errorMatchers: ['Failed to fetch player profile'],
		whatHappened: defineMessage({
			id: 'app.auth-error.no-profile.what',
			defaultMessage:
				'Minecraft services could not return a Java Edition profile for this account. This most often happens when the game was purchased recently, the Java profile has not finished being created, or the wrong Microsoft account is being used.',
		}),
		stepsToFix: [
			signInOfficialLauncher,
			defineMessage({
				id: 'app.auth-error.no-profile.step-launch-once',
				defaultMessage: 'Launch Minecraft: Java Edition once from the official launcher',
			}),
			defineMessage({
				id: 'app.auth-error.no-profile.step-wait',
				defaultMessage: 'Wait up to an hour if the purchase or profile setup was recent',
			}),
			defineMessage({
				id: 'app.auth-error.no-profile.step-right-account',
				defaultMessage:
					'Make sure you are using the Microsoft account that owns Minecraft. You can check which account owns it at <a href="https://www.minecraft.net/en-us/login">minecraft.net</a>',
			}),
			tryProductAgain,
		],
	},
	{
		matches: (message) =>
			message.includes('error sending request for url (') &&
			[
				'minecraft.net',
				'minecraftservices.com',
				'mojang.com',
				'xbox.com',
				'xboxlive.com',
				'live.com',
				'microsoftonline.com',
			].some((domain) => message.includes(domain)),
		whatHappened: defineMessage({
			id: 'app.auth-error.network.what',
			defaultMessage:
				'{product} could not connect to a Microsoft, Xbox, or Minecraft service needed for sign-in. This is usually caused by a local network, DNS, proxy, firewall, hosts file, VPN, or antivirus issue.',
		}),
		stepsToFix: [
			defineMessage({
				id: 'app.auth-error.network.step-restart',
				defaultMessage: 'Restart {product} and try signing in again',
			}),
			defineMessage({
				id: 'app.auth-error.network.step-connection',
				defaultMessage: 'Check that your internet connection is working',
			}),
			defineMessage({
				id: 'app.auth-error.network.step-firewall',
				defaultMessage:
					'Allow {product} through your firewall, antivirus, proxy, VPN, and hosts file rules',
			}),
			defineMessage({
				id: 'app.auth-error.network.step-other-network',
				defaultMessage:
					'Try a different network or temporarily disable VPN/proxy software if you use one',
			}),
			defineMessage({
				id: 'app.auth-error.network.step-warp',
				defaultMessage:
					'If routing or DNS is the issue, a service like Cloudflare WARP can sometimes help',
			}),
		],
	},
	{
		errorCode: '2148916222',
		whatHappened: defineMessage({
			id: 'app.auth-error.uk-age.what',
			defaultMessage:
				'Your Minecraft/Xbox Live account requires age verification to comply with UK regulations. You must complete this before signing in.',
		}),
		stepsToFix: [
			defineMessage({
				id: 'app.auth-error.uk-age.step-login',
				defaultMessage:
					'Go to the <a href="https://www.minecraft.net/en-us/login">Minecraft Login</a> page and sign in',
			}),
			defineMessage({
				id: 'app.auth-error.uk-age.step-verify',
				defaultMessage: 'Follow the instructions to verify your age',
			}),
			defineMessage({
				id: 'app.auth-error.uk-age.step-retry',
				defaultMessage: 'Once verified, try signing in again',
			}),
			defineMessage({
				id: 'app.auth-error.uk-age.step-help',
				defaultMessage:
					'For additional help, visit <a href="https://support.xbox.com/en-GB/help/family-online-safety/online-safety/UK-age-verification">UK age verification on Xbox</a>',
			}),
		],
	},
	{
		errorCode: '2148916233',
		whatHappened: defineMessage({
			id: 'app.auth-error.no-xbox-profile.what',
			defaultMessage: "This account doesn't have an Xbox profile set up or doesn't own Minecraft.",
		}),
		stepsToFix: [
			defineMessage({
				id: 'app.auth-error.no-xbox-profile.step-purchased',
				defaultMessage: 'Make sure Minecraft is purchased on this account',
			}),
			visitMinecraftLogin,
			defineMessage({
				id: 'app.auth-error.no-xbox-profile.step-setup',
				defaultMessage: 'Complete Xbox profile setup if prompted',
			}),
			onceFinishedTryAgain,
		],
	},
	{
		errorCode: '2148916235',
		whatHappened: defineMessage({
			id: 'app.auth-error.region-blocked.what',
			defaultMessage: "Xbox Live isn't available in your region, so sign-in is blocked.",
		}),
		stepsToFix: [
			defineMessage({
				id: 'app.auth-error.region-blocked.step-supported',
				defaultMessage: 'Xbox services must be supported in your country before you can sign in',
			}),
			defineMessage({
				id: 'app.auth-error.region-blocked.step-availability',
				defaultMessage:
					'Check <a href="https://www.xbox.com/en-US/regions">Xbox Availability</a> for supported regions',
			}),
		],
	},
	{
		errorCode: '2148916236',
		whatHappened: defineMessage({
			id: 'app.auth-error.kr-age.what',
			defaultMessage: 'This account requires adult verification under South Korean regulations.',
		}),
		stepsToFix: [
			visitXboxSignIn,
			defineMessage({
				id: 'app.auth-error.kr-age.step-verify',
				defaultMessage: 'Complete the identity verification process',
			}),
			onceFinishedTryAgain,
		],
	},
	{
		errorCode: '2148916237',
		whatHappened: defineMessage({
			id: 'app.auth-error.kr-age.what',
			defaultMessage: 'This account requires adult verification under South Korean regulations.',
		}),
		stepsToFix: [
			visitXboxSignIn,
			defineMessage({
				id: 'app.auth-error.kr-age.step-verify',
				defaultMessage: 'Complete the identity verification process',
			}),
			onceFinishedTryAgain,
		],
	},
	{
		errorCode: '2148916238',
		whatHappened: defineMessage({
			id: 'app.auth-error.underage.what',
			defaultMessage: 'This account is underage and not linked to a Microsoft family group.',
		}),
		stepsToFix: [
			defineMessage({
				id: 'app.auth-error.underage.step-guide',
				defaultMessage:
					'Review the <a href="https://help.minecraft.net/hc/en-us/articles/4408968616077">Family Setup Guide</a>',
			}),
			defineMessage({
				id: 'app.auth-error.underage.step-join',
				defaultMessage: 'Join or create a family group as instructed',
			}),
			onceFinishedTryAgain,
		],
	},
	{
		errorCode: '2148916227',
		whatHappened: defineMessage({
			id: 'app.auth-error.suspended.what',
			defaultMessage: 'This account was suspended for violating Xbox Community Standards.',
		}),
		stepsToFix: [
			defineMessage({
				id: 'app.auth-error.suspended.step-support',
				defaultMessage:
					'Visit <a href="https://support.xbox.com">Xbox Support</a> and review the enforcement details',
			}),
			defineMessage({
				id: 'app.auth-error.suspended.step-appeal',
				defaultMessage: 'Submit an appeal if one is available',
			}),
		],
	},
	{
		errorCode: '2148916229',
		whatHappened: defineMessage({
			id: 'app.auth-error.restricted.what',
			defaultMessage: "This account is restricted and doesn't have permission to play online.",
		}),
		stepsToFix: [
			defineMessage({
				id: 'app.auth-error.restricted.step-family',
				defaultMessage:
					'Have a guardian sign in to <a href="https://account.microsoft.com/family/">Microsoft Family</a>',
			}),
			defineMessage({
				id: 'app.auth-error.restricted.step-permissions',
				defaultMessage: 'Update online play permissions',
			}),
			onceFinishedTryAgain,
		],
	},
	{
		errorCode: '2148916234',
		whatHappened: defineMessage({
			id: 'app.auth-error.tos.what',
			defaultMessage: "This account hasn't accepted Xbox's Terms of Service.",
		}),
		stepsToFix: [
			visitXboxSignIn,
			defineMessage({
				id: 'app.auth-error.tos.step-accept',
				defaultMessage: 'Accept the Terms if prompted',
			}),
			onceFinishedTryAgain,
		],
	},
	{
		errorMatchers: ['Failed to deserialize response to JSON during step XstsAuthorize:'],
		whatHappened: defineMessage({
			id: 'app.auth-error.xsts.what',
			defaultMessage:
				'Xbox services rejected the request to authorize this account for Minecraft services, but did not return a specific account restriction that {product} recognizes.',
		}),
		stepsToFix: [
			signInOfficialLauncher,
			defineMessage({
				id: 'app.auth-error.xsts.step-prompts',
				defaultMessage: 'Complete any prompts shown by Microsoft, Xbox, or Minecraft',
			}),
			tryProductAgain,
			defineMessage({
				id: 'app.auth-error.xsts.step-official-fails',
				defaultMessage:
					'If the official launcher also fails, follow the error shown there or contact Xbox Support',
			}),
		],
	},
]

export function findMinecraftAuthError(message: string): MinecraftAuthError | null {
	return (
		minecraftAuthErrors.find((error) => {
			if (error.errorCode && message.includes(error.errorCode)) {
				return true
			}

			if (error.errorMatchers?.some((matcher) => message.includes(matcher))) {
				return true
			}

			return error.matches?.(message) ?? false
		}) ?? null
	)
}
