import { useCallback, useState } from 'react'
import { HttpError } from '@/http'

export function useRun(onDone?: () => void) {
  const [error, setError] = useState<string | null>(null)

  const run = useCallback(
    async (fn: () => Promise<unknown>) => {
      setError(null)
      try {
        await fn()
        onDone?.()
        return true
      } catch (e) {
        setError(e instanceof HttpError ? e.message : String(e))
        return false
      }
    },
    [onDone],
  )

  return { error, run, setError }
}
