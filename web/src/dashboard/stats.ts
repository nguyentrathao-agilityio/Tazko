import type { BoardPerson, CardData, Label, ListData } from '@/board/types'
import { labelColors } from '@/card/look'

const DAY = 86_400

export type Bar = { key: string; name: string; value: number; color?: string; person?: string }

export type DueBucket = 'overdue' | 'day' | 'week' | 'later' | 'none' | 'done'

export const dueBuckets: { key: DueBucket; color: string }[] = [
  { key: 'overdue', color: 'var(--bad)' },
  { key: 'day', color: 'var(--warn)' },
  { key: 'week', color: 'var(--accent)' },
  { key: 'later', color: 'var(--muted)' },
  { key: 'none', color: 'var(--line)' },
  { key: 'done', color: 'var(--ok)' },
]

export function dueBucket(card: CardData, now: number): DueBucket {
  if (card.done) return 'done'
  const due = card.due_at
  if (!due) return 'none'
  if (due < now) return 'overdue'
  if (due - now < DAY) return 'day'
  if (due - now < 7 * DAY) return 'week'
  return 'later'
}

export type Stats = {
  cards: number
  done: number
  overdue: number
  soon: number
  lists: Bar[]
  members: Bar[]
  labels: Bar[]
  due: Record<DueBucket, number>
}

export function boardStats(lists: ListData[], people: BoardPerson[], labels: Label[], now = Date.now() / 1000): Stats {
  const cards = lists.flatMap((l) => l.cards)
  const count = (test: (c: CardData) => boolean) => cards.filter(test).length
  const due = Object.fromEntries(dueBuckets.map((b) => [b.key, 0])) as Record<DueBucket, number>
  for (const c of cards) due[dueBucket(c, now)] += 1

  return {
    cards: cards.length,
    done: due.done,
    overdue: due.overdue,
    soon: count((c) => !c.done && Boolean(c.due_at) && c.due_at! >= now && c.due_at! - now < 2 * DAY),
    lists: lists.map((l) => ({ key: `l${l.id}`, name: l.title, value: l.cards.length })),
    members: [
      ...people.map((p) => ({ key: `p${p.id}`, name: p.name || p.email, person: p.name || p.email, value: count((c) => Boolean(c.members?.includes(p.id))) })),
      { key: 'none', name: '', value: count((c) => !c.members?.length) },
    ],
    labels: labels.map((l) => ({ key: `b${l.id}`, name: l.name, value: count((c) => Boolean(c.labels?.includes(l.id))), color: labelColors[l.color] ?? labelColors[0] })),
    due,
  }
}
