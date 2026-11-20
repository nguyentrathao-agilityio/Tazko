import { useCallback, useRef } from 'react'

export function useCardToggle() {
  const pending = useRef(new Set<number>())
  const expected = useRef(new Map<number, boolean>())
  return useCallback((id: number, actual: boolean, send: (next: boolean) => Promise<unknown>) => {
    if (pending.current.has(id)) return
    const known = expected.current.get(id)
    const current = known === undefined || known === actual ? actual : known
    const next = !current
    expected.current.set(id, next)
    pending.current.add(id)
    void send(next).finally(() => pending.current.delete(id))
  }, [])
}
