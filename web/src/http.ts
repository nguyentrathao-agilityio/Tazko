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

export async function api<T>(method: string, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = { accept: 'application/json' }
  if (method !== 'GET') headers['x-csrf-token'] = csrf
  if (body !== undefined) headers['content-type'] = 'application/json'
  const res = await fetch(path, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) })
  if (!res.ok) {
    const data = (await res.json().catch(() => ({}))) as { error?: string }
    throw new HttpError(res.status, data.error ?? `request failed (${res.status})`)
  }

  if (res.status === 204) return undefined as T
  return (await res.json()) as T
}

export async function upload<T>(path: string, file: File): Promise<T> {
  const res = await fetch(`${path}?name=${encodeURIComponent(file.name)}`, {
    method: 'POST',
    headers: { accept: 'application/json', 'x-csrf-token': csrf, 'content-type': 'application/octet-stream' },
    body: file,
  })
  if (!res.ok) {
    const data = (await res.json().catch(() => ({}))) as { error?: string }
    throw new HttpError(res.status, data.error ?? (res.status === 413 ? 'this file is too large' : `upload failed (${res.status})`))
  }

  return (await res.json()) as T
}
