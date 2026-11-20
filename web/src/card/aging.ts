const DAY = 86_400

export type Aging = { on: boolean; days: number }

export function quietDays(updated: number | undefined, now = Date.now() / 1000): number {
  return updated ? Math.floor((now - updated) / DAY) : 0
}

export function isStale(card: { done?: boolean; updated_at?: number }, days: number, now = Date.now() / 1000): boolean {
  return !card.done && Boolean(card.updated_at) && now - (card.updated_at ?? now) >= days * DAY
}

export function ageLevel(card: { done?: boolean; updated_at?: number }, aging: Aging | undefined, now = Date.now() / 1000): 0 | 1 | 2 | 3 {
  if (!aging?.on || card.done || !card.updated_at) return 0
  const age = now - card.updated_at
  const t = aging.days * DAY
  return age >= 3 * t ? 3 : age >= 2 * t ? 2 : age >= t ? 1 : 0
}
