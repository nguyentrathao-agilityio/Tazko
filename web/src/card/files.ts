import type { Attachment } from '@/card/types'

const KB = 1024
const MB = KB ** 2
const GB = KB ** 3

function short(n: number): string {
  return n >= 10 || Number.isInteger(n) ? String(Math.round(n * 10) / 10).replace(/\.0$/, '') : n.toFixed(1).replace(/\.0$/, '')
}

export function sizeShort(n: number): string {
  if (n >= GB) return `${short(n / GB)} GB`
  if (n >= MB) return `${short(n / MB)} MB`
  return `${Math.max(1, Math.round(n / KB))} KB`
}

export function usage(used: number, limit: number): string {
  const [unit, size] = limit >= GB ? ['GB', GB] : ['MB', MB]
  const u = used / size
  return `${u < 0.1 ? sizeShort(used) : short(u)} of ${short(limit / size)} ${unit}`
}

export function fileSize(n: number): string {
  if (n >= GB) return `${(n / GB).toFixed(1)} GB`
  if (n >= MB) return `${(n / MB).toFixed(1)} MB`
  if (n >= KB) return `${Math.round(n / KB)} KB`
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
