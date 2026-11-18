import { getSettings } from '@/settings/store'

export const labelColors = ['#4B4FD8', '#0F8F7E', '#D6456B', '#D33F3F', '#E0A020', '#5E6A85', '#3E9B4F', '#E2762B']

export function labelStyle(color: number) {
  return { background: labelColors[color] ?? labelColors[0], color: color === 4 ? '#2b2100' : '#fff' }
}

const DAY = 86_400

export function dueState(due: number | null | undefined, done: boolean | undefined, now = Date.now() / 1000): '' | 'ok' | 'warn' | 'bad' {
  if (!due) return ''
  if (done) return 'ok'
  if (due < now) return 'bad'
  if (due - now < DAY) return 'warn'
  return ''
}

const formats = new Map<string, Intl.DateTimeFormat>()

function formatter(time: boolean): Intl.DateTimeFormat {
  const s = getSettings()
  const key = `${s.time_zone}|${s.date_format}|${s.time_format}|${time}`
  const found = formats.get(key)
  if (found) return found
  const zone = (() => {
    try {
      new Intl.DateTimeFormat('en-US', { timeZone: s.time_zone })
      return s.time_zone
    } catch {
      return undefined
    }
  })()
  const locale = s.date_format === 'ISO' ? 'sv-SE' : s.date_format === 'DMY' ? 'en-GB' : 'en-US'
  const day: Intl.DateTimeFormatOptions = s.date_format === 'ISO' ? { year: 'numeric', month: '2-digit', day: '2-digit' } : { month: 'short', day: 'numeric' }
  const clock: Intl.DateTimeFormatOptions = time ? { hour: 'numeric', minute: '2-digit', hour12: s.time_format === '12' } : {}
  const f = new Intl.DateTimeFormat(locale, { ...day, ...clock, timeZone: zone })
  formats.set(key, f)
  return f
}

export function shortDate(unix: number, time = false): string {
  return formatter(time).format(new Date(unix * 1000))
}

export function dateRange(start: number | null | undefined, due: number | null | undefined, time = false): string {
  if (start && due) return `${shortDate(start)} – ${shortDate(due, time)}`
  if (due) return shortDate(due, time)
  if (start) return `Starts ${shortDate(start)}`
  return ''
}

export function ago(unix: number, now = Date.now()): string {
  const m = Math.round((now - unix * 1000) / 60_000)
  if (m < 1) return 'Just now'
  if (m < 60) return `${m} min ago`
  const h = Math.round(m / 60)
  if (h < 24) return `${h} h ago`
  return `${Math.round(h / 24)} d ago`
}
