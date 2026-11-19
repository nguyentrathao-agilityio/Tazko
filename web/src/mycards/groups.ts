import type { MyCard } from '@/mycards/types'

const DAY = 86_400

export const groupOrder = ['overdue', 'today', 'week', 'later', 'none', 'done'] as const

export type Group = (typeof groupOrder)[number]

export function groupOf(card: MyCard, now: number): Group {
  if (card.done) return 'done'
  if (!card.due_at) return 'none'
  if (card.due_at < now) return 'overdue'
  if (card.due_at - now < DAY) return 'today'
  if (card.due_at - now < 7 * DAY) return 'week'
  return 'later'
}

export const projectKey = (c: MyCard) => `${c.company.slug}:${c.project.id}`

export function groupCards(cards: MyCard[], now = Date.now() / 1000): [Group, MyCard[]][] {
  const by = new Map<Group, MyCard[]>()
  for (const c of cards) {
    const g = groupOf(c, now)
    by.set(g, [...(by.get(g) ?? []), c])
  }

  return groupOrder.flatMap((g) => {
    const list = by.get(g)
    return list ? [[g, [...list].sort((a, b) => (a.due_at ?? Infinity) - (b.due_at ?? Infinity) || a.id - b.id)] as [Group, MyCard[]]] : []
  })
}
