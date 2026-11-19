import type { BoardPerson, Label, ListData } from '@/board/types'
import { DAY, dueBucket, dueBucketOrder, labelColors, type DueBucket } from '@/card/look'

export type Bar = { key: string; name: string; value: number; color?: string; person?: string }

const dueColors: Record<DueBucket, string> = {
  overdue: 'var(--bad)',
  day: 'var(--warn)',
  week: 'var(--accent)',
  later: 'var(--muted)',
  none: 'var(--line)',
  done: 'var(--ok)',
}

export type Stats = {
  cards: number
  done: number
  overdue: number
  soon: number
  lists: Bar[]
  members: Bar[]
  labels: Bar[]
  due: Bar[]
}

export function boardStats(lists: ListData[], people: BoardPerson[], labels: Label[], dueName: (b: DueBucket) => string, unassigned: string, now = Date.now() / 1000): Stats {
  const due = new Map<DueBucket, number>()
  const byMember = new Map<number, number>()
  const byLabel = new Map<number, number>()
  const known = new Set(people.map((p) => p.id))
  const bump = <K>(m: Map<K, number>, k: K) => m.set(k, (m.get(k) ?? 0) + 1)
  let cards = 0
  let soon = 0
  let none = 0
  for (const c of lists.flatMap((l) => l.cards)) {
    cards += 1
    bump(due, dueBucket(c.due_at, c.done, now))
    if (!c.done && c.due_at && c.due_at >= now && c.due_at - now < 2 * DAY) soon += 1
    const members = (c.members ?? []).filter((m) => known.has(m))
    for (const m of members) bump(byMember, m)
    for (const l of c.labels ?? []) bump(byLabel, l)
    if (!members.length) none += 1
  }

  return {
    cards,
    done: due.get('done') ?? 0,
    overdue: due.get('overdue') ?? 0,
    soon,
    lists: lists.map((l) => ({ key: `l${l.id}`, name: l.title, value: l.cards.length })),
    members: [
      ...people.map((p) => ({ key: `p${p.id}`, name: p.name || p.email, person: p.name || p.email, value: byMember.get(p.id) ?? 0 })),
      { key: 'none', name: unassigned, value: none },
    ],
    labels: labels.map((l) => ({ key: `b${l.id}`, name: l.name, value: byLabel.get(l.id) ?? 0, color: labelColors[l.color] ?? labelColors[0] })),
    due: dueBucketOrder.map((b) => ({ key: b, name: dueName(b), value: due.get(b) ?? 0, color: dueColors[b] })),
  }
}
