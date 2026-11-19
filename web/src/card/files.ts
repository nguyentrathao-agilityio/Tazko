import type { Attachment } from '@/card/types'

function short(n: number): string {
  return n >= 10 || Number.isInteger(n) ? String(Math.round(n * 10) / 10).replace(/\.0$/, '') : n.toFixed(1).replace(/\.0$/, '')
}

export function sizeShort(n: number): string {
  if (n >= 1024 ** 3) return `${short(n / 1024 ** 3)} GB`
  if (n >= 1024 ** 2) return `${short(n / 1024 ** 2)} MB`
  return `${Math.max(1, Math.round(n / 1024))} KB`
}

export function usage(used: number, limit: number): string {
  const [unit, size] = limit >= 1024 ** 3 ? ['GB', 1024 ** 3] : ['MB', 1024 ** 2]
  const u = used / size
  return u < 0.1 ? `${sizeShort(used)} of ${short(limit / size)} ${unit}` : `${short(u)} of ${short(limit / size)} ${unit}`
}

export function fileSize(n: number): string {
  if (n >= 1024 ** 3) return `${(n / 1024 ** 3).toFixed(1)} GB`
  if (n >= 1024 ** 2) return `${(n / 1024 ** 2).toFixed(1)} MB`
  if (n >= 1024) return `${Math.round(n / 1024)} KB`
  return `${n} B`
}

export function shortUrl(u: string): string {
  const s = u.replace(/^https?:\/\/(www\.)?/, '').replace(/\/$/, '')
  return s.length > 44 ? `${s.slice(0, 30)}…${s.slice(-10)}` : s
}

export function shownName(a: Attachment): string {
  return /^https?:\/\//.test(a.name) ? shortUrl(a.name) : a.name
}

export function extOf(name: string): string {
  const i = name.lastIndexOf('.')
  return i > 0 ? name.slice(i + 1).toUpperCase().slice(0, 4) : 'FILE'
}

export function fileUrl(base: string, id: number, download = false): string {
  return `${base}/attachments/${id}${download ? '?download=1' : ''}`
}
