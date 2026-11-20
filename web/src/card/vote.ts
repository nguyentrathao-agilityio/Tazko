import { useCallback, useRef } from 'react'
import { api } from '@/http'

export function useVote(run: (fn: () => Promise<unknown>) => Promise<boolean>) {
  const pending = useRef(new Set<number>())
  return useCallback(
    (boardApi: string, card: { id: number; voted?: boolean }) => {
      if (pending.current.has(card.id)) return
      pending.current.add(card.id)
      void run(() => api('PUT', `${boardApi}/cards/${card.id}/vote`, { on: !card.voted })).finally(() => pending.current.delete(card.id))
    },
    [run],
  )
}
