import adapter from '@sveltejs/adapter-static'
import precompileIntl from 'svelte-intl-precompile/sveltekit-plugin'
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'

export default {
	preprocess: vitePreprocess([precompileIntl('locales')]),
	kit: {
		csp: {
			mode: 'hash',
			directives: {
				'default-src': ['self'],
				'base-uri': ['none'],
				'connect-src': ['self'],
				'font-src': ['self'],
				'form-action': ['self'],
				'frame-src': ['none'],
				'img-src': ['self', 'data:', 'blob:'],
				'manifest-src': ['self'],
				'media-src': ['self', 'blob:'],
				'object-src': ['none'],
				'script-src': ['self'],
				'style-src': ['self'],
				'worker-src': ['none'],
			},
		},
		adapter: adapter({
			fallback: 'index.html',
		}),
	},
}
