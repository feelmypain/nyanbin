import { expect, test } from '@playwright/test'
import { OPS_SERVER, SERVER, waitReady } from '../utils'

// CORS is negotiated by the server, not the browser under test; run once, on Chromium.
test.skip(({ browserName }) => browserName !== 'chromium', 'API contract specs run once, on Chromium')

const ALLOWED_ORIGIN = 'https://allowed.example'

async function preflight(server: string, origin: string): Promise<Response> {
  return fetch(`${server}/api/status`, {
    method: 'OPTIONS',
    headers: {
      origin,
      'access-control-request-method': 'GET',
      'access-control-request-headers': 'content-type',
    },
  })
}

test.describe('CORS disabled (default instance)', () => {
  test('preflight yields no access-control-allow-origin', async () => {
    const response = await preflight(SERVER, ALLOWED_ORIGIN)
    expect(response.headers.get('access-control-allow-origin')).toBeNull()
  })

  test('simple cross-origin GET yields no access-control-allow-origin', async () => {
    const response = await fetch(`${SERVER}/api/status`, { headers: { origin: ALLOWED_ORIGIN } })
    expect(response.status).toBe(200)
    expect(response.headers.get('access-control-allow-origin')).toBeNull()
  })

  test('API responses are marked same-origin resources', async () => {
    const response = await fetch(`${SERVER}/api/status`)
    expect(response.headers.get('cross-origin-resource-policy')).toBe('same-origin')
  })
})

test.describe('CORS allowlist instance', () => {
  test.beforeAll(async () => {
    await waitReady(OPS_SERVER)
  })

  test('preflight from an allowed origin grants exactly that origin', async () => {
    const response = await preflight(OPS_SERVER, ALLOWED_ORIGIN)
    expect(response.headers.get('access-control-allow-origin')).toBe(ALLOWED_ORIGIN)

    const methods = (response.headers.get('access-control-allow-methods') ?? '')
      .split(',')
      .map((method) => method.trim().toUpperCase())
    for (const method of ['GET', 'POST', 'PUT', 'DELETE']) expect(methods).toContain(method)

    const headers = (response.headers.get('access-control-allow-headers') ?? '').toLowerCase()
    expect(headers).toContain('content-type')
    expect(response.headers.get('access-control-max-age')).toBe('600')
    // Zero-knowledge API: no cookies, no credentialed CORS. Ever.
    expect(response.headers.get('access-control-allow-credentials')).toBeNull()
  })

  test('preflight from a foreign origin is not granted', async () => {
    const response = await preflight(OPS_SERVER, 'https://evil.example')
    expect(response.headers.get('access-control-allow-origin')).toBeNull()
  })

  test('simple GET from the allowed origin echoes the origin and varies on it', async () => {
    const response = await fetch(`${OPS_SERVER}/api/status`, { headers: { origin: ALLOWED_ORIGIN } })
    expect(response.status).toBe(200)
    expect(response.headers.get('access-control-allow-origin')).toBe(ALLOWED_ORIGIN)
    const vary = (response.headers.get('vary') ?? '').toLowerCase()
    expect(vary).toContain('origin')
  })

  test('API responses are marked cross-origin resources', async () => {
    const response = await fetch(`${OPS_SERVER}/api/status`)
    expect(response.headers.get('cross-origin-resource-policy')).toBe('cross-origin')
  })
})
