import { useCallback } from 'react'
import { api } from '@/http'
import { useCardToggle } from '@/card/toggle'

export function useVote(run: (fn: () => Promise<unknown>) => Promise<boolean>) {
  const toggle = useCardToggle()
  return useCallback(
    (boardApi: string, card: { id: number; voted?: boolean }) => toggle(card.id, Boolean(card.voted), (on) => run(() => api('PUT', `${boardApi}/cards/${card.id}/vote`, { on }))),
    [run, toggle],
  )
}
