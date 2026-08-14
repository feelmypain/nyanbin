const script = `try{const value=localStorage.getItem('nyanbin-theme');if(value==='light'||value==='dark')document.documentElement.setAttribute('theme',value)}catch{}`

export const prerender = true

export function GET() {
	return new Response(script, {
		headers: {
			'content-type': 'text/javascript; charset=utf-8',
			'cache-control': 'no-cache',
		},
	})
}
