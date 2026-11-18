const time = new Intl.DateTimeFormat('en-GB', { hour: '2-digit', minute: '2-digit' })
const day = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'short' })
const dayYear = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })

export function when(unix: number | null | undefined, now: Date = new Date()): string {
  if (!unix) return '—'
  const d = new Date(unix * 1000)
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()
  const ms = d.getTime()
  if (ms >= start) return `Today ${time.format(d)}`
  if (ms >= start - 86_400_000) return `Yesterday ${time.format(d)}`
  if (d.getFullYear() === now.getFullYear()) return `${day.format(d)} ${time.format(d)}`
  return dayYear.format(d)
}

export function bytes(n: number | null | undefined): string {
  if (n == null) return '—'
  const gb = n / 1024 ** 3
  if (gb >= 1) return `${gb.toFixed(gb >= 10 ? 0 : 1)} GB`
  const mb = n / 1024 ** 2
  if (mb >= 1) return `${mb.toFixed(mb >= 10 ? 0 : 1)} MB`
  return `${Math.max(1, Math.round(n / 1024))} KB`
}

export function duration(secs: number): string {
  const d = Math.floor(secs / 86_400)
  const h = Math.floor((secs % 86_400) / 3600)
  const m = Math.floor((secs % 3600) / 60)
  if (d > 0) return `${d} day${d === 1 ? '' : 's'} ${h} h`
  if (h > 0) return `${h} h ${m} min`
  return `${m} min`
}
