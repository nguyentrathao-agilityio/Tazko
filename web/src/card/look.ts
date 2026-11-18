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

const short = new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric' })
const withTime = new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' })

export function shortDate(unix: number, time = false): string {
  return (time ? withTime : short).format(new Date(unix * 1000))
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
