import { boardApiOf } from '@/paths'

export const backgrounds: Record<string, string> = {
  dusk: 'linear-gradient(135deg,#4b4fd8,#8b5cf6 60%,#c084fc)',
  sea: 'linear-gradient(135deg,#0ea5a4,#2563eb)',
  sand: 'linear-gradient(135deg,#f59e0b,#ef7a5a)',
  meadow: 'linear-gradient(135deg,#16a34a,#84cc16)',
  rose: 'linear-gradient(135deg,#e11d74,#f97316)',
  slate: 'linear-gradient(135deg,#334155,#64748b)',
  mist: 'linear-gradient(135deg,#8a94e8,#c4a8ee)',
  ink: '#1f2547',
  leaf: '#2f855a',
  coral: '#d9534f',
  ocean: '#1e6fb8',
}

export const backgroundNames = Object.keys(backgrounds)

export const colors = ['c0', 'c1', 'c2', 'c3', 'c4']

export function background(name: string, board?: string): string {
  const m = /^img:(\d+)$/.exec(name)
  if (m) return board ? `url("${board}/background/${m[1]}") center / cover no-repeat, ${backgrounds.ink}` : backgrounds.ink
  return backgrounds[name] ?? backgrounds.dusk
}

export function boardApi(board: number): string | undefined {
  const m = /^\/c\/([a-z0-9-]+)(\/|$)/.exec(window.location.pathname)
  return m ? boardApiOf(m[1], board) : undefined
}
