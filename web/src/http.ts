export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'

const JSON_TYPE = 'application/json'
export const CSRF_HEADER = 'x-csrf-token'
export const ACCEPT_JSON = { accept: JSON_TYPE } as const

const BINARY_TYPE = 'application/octet-stream'

let csrf = ''

export function setCsrf(token: string) {
  csrf = token
}

export class HttpError extends Error {
  status: number

  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

export function errorMessage(e: unknown): string {
  return e instanceof HttpError ? e.message : String(e)
}

async function errorFrom(res: Response, fallback: string): Promise<HttpError> {
  const data = (await res.json().catch(() => ({}))) as { error?: string }
  return new HttpError(res.status, data.error ?? fallback)
}

export async function api<T>(method: HttpMethod, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = { ...ACCEPT_JSON }
  if (method !== 'GET') headers[CSRF_HEADER] = csrf
  if (body !== undefined) headers['content-type'] = JSON_TYPE
  const res = await fetch(path, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) })
  if (!res.ok) throw await errorFrom(res, `request failed (${res.status})`)
  if (res.status === 204) return undefined as T
  return (await res.json()) as T
}

async function sendFile<T>(url: string, file: Blob, headers: Record<string, string>): Promise<T> {
  const res = await fetch(url, {
    method: 'POST',
    headers: { ...ACCEPT_JSON, [CSRF_HEADER]: csrf, ...headers },
    body: file,
  })
  if (!res.ok) throw await errorFrom(res, res.status === 413 ? 'this file is too large' : `upload failed (${res.status})`)
  return (await res.json()) as T
}

export function upload<T>(path: string, file: File): Promise<T> {
  return sendFile<T>(`${path}?name=${encodeURIComponent(file.name)}`, file, { 'content-type': BINARY_TYPE })
}

export function sendJsonFile<T>(url: string, file: Blob): Promise<T> {
  return sendFile<T>(url, file, { 'content-type': JSON_TYPE })
}
