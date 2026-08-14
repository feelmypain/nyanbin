import { writable } from 'svelte/store'

/** Set while a page holds an unsaved bearer link; invoking the callback leaves that page. */
export const leaveGuard = writable<(() => void) | null>(null)
