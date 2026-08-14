import { AES_GCM_TAG_BYTES, ENVELOPE_HEADER_BYTES, type PrivatePayload } from 'nyanbin/shared'

const encoder = new TextEncoder()

export async function copy(value: string): Promise<boolean> {
	try {
		await navigator.clipboard.writeText(value)
		return true
	} catch {
		return false
	}
}

export function formatExpiry(expiresAt: number, locale?: string): string {
	return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(expiresAt))
}

export type EnvelopeFile = Pick<File, 'name' | 'type' | 'size'>

function base64UrlLength(byteLength: number): number {
	return Math.ceil(byteLength / 3) * 4 - (byteLength % 3 === 0 ? 0 : 3 - (byteLength % 3))
}
export function serializedTextBytes(text: string): number {
	return encoder.encode(JSON.stringify(text)).byteLength - 2
}

export function envelopeFileBytes(file: EnvelopeFile): number {
	const metadata = JSON.stringify({
		name: file.name,
		type: file.type || 'application/octet-stream',
		size: file.size,
		data: '',
	})
	return encoder.encode(metadata).byteLength + base64UrlLength(file.size)
}

export function encryptedEnvelopeBytes(text: string, format: PrivatePayload['format'], files: EnvelopeFile[]): number {
	const serializedWithoutFileData = JSON.stringify({
		kind: 'text',
		format,
		text,
		files: files.map((file) => ({
			name: file.name,
			type: file.type || 'application/octet-stream',
			size: file.size,
			data: '',
		})),
	})
	const fileDataBytes = files.reduce((total, file) => total + base64UrlLength(file.size), 0)
	return ENVELOPE_HEADER_BYTES + AES_GCM_TAG_BYTES + encoder.encode(serializedWithoutFileData).byteLength + fileDataBytes
}

export function safeFilename(name: string, fallback = 'download.bin'): string {
	const neutralized = name
		.replace(/[\\/\p{Cc}\p{Cf}\p{Zl}\p{Zp}]/gu, '_')
		.replace(/^[\s.]+|[\s.]+$/gu, '_')
	const bounded = Array.from(neutralized).slice(0, 255).join('')
	return bounded || fallback
}
