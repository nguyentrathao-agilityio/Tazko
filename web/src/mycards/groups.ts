import { dueBucket, dueBucketOrder, type DueBucket } from '@/card/look'
import type { MyCard } from '@/mycards/types'

export const projectKey = (c: MyCard) => `${c.company.slug}:${c.project.id}`

export function groupCards(cards: MyCard[], now = Date.now() / 1000): [DueBucket, MyCard[]][] {
  const by = new Map<DueBucket, MyCard[]>()
  for (const c of cards) {
    const g = dueBucket(c.due_at, c.done, now)
    const list = by.get(g)
    if (list) list.push(c)
    else by.set(g, [c])
  }

  return dueBucketOrder.flatMap((g) => {
    const list = by.get(g)
    return list ? [[g, list.sort((a, b) => (a.due_at ?? Infinity) - (b.due_at ?? Infinity) || a.id - b.id)] as [DueBucket, MyCard[]]] : []
  })
}
