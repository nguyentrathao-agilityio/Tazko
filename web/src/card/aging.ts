import { DAY } from '@/card/look'

export type Aging = { on: boolean; days: number }

export const AGING_DAYS = [3, 7, 14, 30] as const

export function agingOf(board: { aging?: boolean; aging_days?: number }): Aging {
  return { on: Boolean(board.aging), days: board.aging_days ?? 7 }
}

type Aged = { done?: boolean; updated_at?: number; is_template?: boolean }

export function quietDays(updated: number | undefined, now = Date.now() / 1000): number {
  return updated ? Math.floor((now - updated) / DAY) : 0
}

export function ageLevel(card: Aged, aging: Aging | undefined, now = Date.now() / 1000): 0 | 1 | 2 | 3 {
  if (!aging?.on || card.done || card.is_template || !card.updated_at) return 0
  const age = now - card.updated_at
  const t = aging.days * DAY
  return age >= 3 * t ? 3 : age >= 2 * t ? 2 : age >= t ? 1 : 0
}

export function isStale(card: Aged, aging: Aging | undefined, now = Date.now() / 1000): boolean {
  return ageLevel(card, aging, now) > 0
}
