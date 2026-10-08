import {
	AbstractWebNotificationManager,
	type NotificationPanelLocation,
	type WebNotification,
} from '@orbiont/ui'
import { type Ref, ref } from 'vue'

export class AppNotificationManager extends AbstractWebNotificationManager {
	private readonly state: Ref<WebNotification[]>
	private readonly locationState: Ref<NotificationPanelLocation>
	private readonly curseforgeLimitNotification: () => Pick<WebNotification, 'title' | 'text'>

	public constructor(curseforgeLimitNotification: () => Pick<WebNotification, 'title' | 'text'>) {
		super()
		this.curseforgeLimitNotification = curseforgeLimitNotification
		this.state = ref<WebNotification[]>([])
		this.locationState = ref<NotificationPanelLocation>('right')
	}

	// The shared upstream limit is one incident, regardless of the requested path.
	override handleError = (error: unknown): void => {
		const text = error instanceof Error ? error.message : String(error)
		if (/CurseForge/i.test(text) && /\b429\b/.test(text)) {
			this.addNotification({
				...this.curseforgeLimitNotification(),
				type: 'warning',
				supportData: { cause: text },
			})
			return
		}
		this.addNotification({ title: 'An error occurred', text, type: 'error' })
	}

	public getNotificationLocation(): NotificationPanelLocation {
		return this.locationState.value
	}

	public setNotificationLocation(location: NotificationPanelLocation): void {
		this.locationState.value = location
	}

	public getNotifications(): WebNotification[] {
		return this.state.value
	}

	protected addNotificationToStorage(notification: WebNotification): void {
		this.state.value.push(notification)
	}

	protected removeNotificationFromStorage(id: string | number): void {
		const index = this.state.value.findIndex((n) => n.id === id)
		if (index > -1) {
			this.state.value.splice(index, 1)
		}
	}

	protected removeNotificationFromStorageByIndex(index: number): void {
		this.state.value.splice(index, 1)
	}

	protected clearAllNotificationsFromStorage(): void {
		this.state.value.splice(0)
	}
}
