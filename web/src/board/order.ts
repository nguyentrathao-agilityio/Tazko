import type { ListData } from '@/board/types'

export function appendAfter(lists: ListData[], list: number, card: number, top = false): number | null {
  if (top) return null
  const others = (lists.find((l) => l.id === list)?.cards ?? []).filter((c) => c.id !== card)
  return others.length ? others[others.length - 1].id : null
}
