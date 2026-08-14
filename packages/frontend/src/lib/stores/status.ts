import { API, type Status } from 'nyanbin/shared'
import { writable } from 'svelte/store'

export type StatusState =
	| { state: 'loading' }
	| { state: 'ready'; value: Status }
	| { state: 'error'; message: string }

export const status = writable<StatusState>({ state: 'loading' })

export async function init() {
	status.set({ state: 'loading' })
	try {
		status.set({ state: 'ready', value: await API.status() })
	} catch {
		status.set({ state: 'error', message: 'status_unavailable' })
	}
}
