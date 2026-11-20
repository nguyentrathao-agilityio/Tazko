import { dueBucket, dueBucketOrder, type DueBucket } from '@/card/look'
type Dated = { id: number; done: boolean; due_at: number | null; company: { slug: string }; project: { id: number } }

export const projectKey = (c: Dated) => `${c.company.slug}:${c.project.id}`

export function groupCards<T extends Dated>(cards: T[], now = Date.now() / 1000): [DueBucket, T[]][] {
  const by = new Map<DueBucket, T[]>()
  for (const c of cards) {
    const g = dueBucket(c.due_at, c.done, now)
    const list = by.get(g)
    if (list) list.push(c)
    else by.set(g, [c])
  }

  return dueBucketOrder.flatMap((g) => {
    const list = by.get(g)
    return list ? [[g, list.sort((a, b) => (a.due_at ?? Infinity) - (b.due_at ?? Infinity) || a.id - b.id)] as [DueBucket, T[]]] : []
  })
}
