import { getLocaleFromNavigator, init, waitLocale } from 'svelte-intl-precompile'
// @ts-ignore
import { availableLocales, registerAll } from '$locales'

registerAll()

function supportedLocale(value: string | null | undefined): string | undefined {
	if (!value) return undefined
	const exact = availableLocales.find((option: string) => option.toLowerCase() === value.toLowerCase())
	if (exact) return exact
	const base = value.split('-')[0]?.toLowerCase()
	return availableLocales.find((option: string) => option.toLowerCase() === base)
}

let storedLocale: string | null = null
if (typeof window !== 'undefined') {
	try { storedLocale = window.localStorage.getItem('nyanbin-locale') } catch { /* Use the navigator fallback. */ }
}
const initialLocale = supportedLocale(storedLocale) ?? supportedLocale(getLocaleFromNavigator()) ?? supportedLocale('en') ?? availableLocales[0]
if (storedLocale !== null && storedLocale !== initialLocale && typeof window !== 'undefined') {
	try { window.localStorage.setItem('nyanbin-locale', initialLocale) } catch { /* The normalized in-memory locale still applies. */ }
}
init({ initialLocale, fallbackLocale: 'en' })

// Waiting here (not in an {#await} in the layout markup) lets prerendering
// emit full page content instead of an empty pending branch.
export const load = async () => {
	await waitLocale()
}
