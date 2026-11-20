import type { CardData, ListData } from '@/board/types'
import { isStale, type Aging } from '@/card/aging'
import { DAY } from '@/card/look'

export type Filter = { q: string; members: string[]; labels: string[]; due: string[]; fields?: string[]; hideDone?: boolean; stale?: boolean; hideStale?: boolean }

export const emptyFilter: Filter = { q: '', members: [], labels: [], due: [] }

export function active(f: Filter): boolean {
  return Boolean(f.q.trim() || f.members.length || f.labels.length || f.due.length || f.fields?.length || f.hideDone || f.stale || f.hideStale)
}


export function matches(c: CardData, f: Filter, me: number, labelNames: Map<number, string>, now = Date.now() / 1000, aging?: Aging): boolean {
  if (f.hideDone && c.done) return false
  if ((f.stale || f.hideStale) && isStale(c, aging, now) !== Boolean(f.stale)) return false
  if (f.q.trim()) {
    const hay = `${c.title} ${(c.labels ?? []).map((id) => labelNames.get(id) ?? '').join(' ')}`.toLowerCase()
    if (!f.q.toLowerCase().split(/\s+/).filter(Boolean).every((w) => hay.includes(w))) return false
  }

  const members = c.members ?? []
  if (f.members.length && !f.members.some((m) => (m === 'none' ? members.length === 0 : m === 'me' ? members.includes(me) : members.includes(Number(m))))) return false
  const labels = c.labels ?? []
  if (f.labels.length && !f.labels.some((l) => (l === 'none' ? labels.length === 0 : labels.includes(Number(l))))) return false
  if (f.due.length) {
    const due = c.due_at ?? 0
    const ok = f.due.some((k) => {
      if (k === 'none') return !due
      if (k === 'over') return Boolean(due && due < now && !c.done)
      if (k === 'day') return Boolean(due && due >= now && due - now < DAY && !c.done)
      if (k === 'week') return Boolean(due && due >= now && due - now < 7 * DAY && !c.done)
      if (k === 'done') return Boolean(c.done)
      if (k === 'open') return !c.done
      return false
    })
    if (!ok) return false
  }

  if (f.fields?.length) {
    const ok = f.fields.every((key) => {
      const at = key.indexOf(':')
      const v = c.fields?.[key.slice(0, at)]
      return key.slice(at + 1) === 'yes' ? v === true : v === key.slice(at + 1)
    })
    if (!ok) return false
  }

  return true
}

export function applyFilter(lists: ListData[], f: Filter, me: number, labelNames: Map<number, string>, aging?: Aging): ListData[] {
  if (!active(f)) return lists
  const now = Date.now() / 1000
  return lists.map((l) => ({ ...l, cards: l.cards.filter((c) => matches(c, f, me, labelNames, now, aging)) }))
}

export function toggleHideStale(f: Filter): Filter {
  return { ...f, hideStale: !f.hideStale, stale: false }
}

export function toggleMine(f: Filter): Filter {
  return { ...f, members: f.members.includes('me') ? f.members.filter((m) => m !== 'me') : [...f.members, 'me'] }
}
