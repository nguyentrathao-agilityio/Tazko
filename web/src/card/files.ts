import type { Attachment } from '@/card/types'

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
