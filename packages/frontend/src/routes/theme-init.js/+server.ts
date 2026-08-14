const script = `try{const value=localStorage.getItem('nyanbin-theme');if(['light','dark','red','green','pink'].includes(value))document.documentElement.setAttribute('theme',value)}catch{}`

export const prerender = true

export function GET() {
	return new Response(script, {
		headers: {
			'content-type': 'text/javascript; charset=utf-8',
			'cache-control': 'no-cache',
		},
	})
}
