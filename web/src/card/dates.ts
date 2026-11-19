const pad = (n: number) => String(n).padStart(2, '0')

export function toDateInput(unix: number | null | undefined): string {
  if (!unix) return ''
  const d = new Date(unix * 1000)
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}

export function toTimeInput(unix: number | null | undefined): string {
  if (!unix) return '12:00'
  const d = new Date(unix * 1000)
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`
}

export function fromInputs(date: string, time: string): number | null {
  if (!date) return null
  const [y, m, d] = date.split('-').map(Number)
  const [hh, mm] = (time || '12:00').split(':').map(Number)
  return Math.floor(new Date(y, m - 1, d, hh, mm).getTime() / 1000)
}
