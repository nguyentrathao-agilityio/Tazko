import { useCallback, useState } from 'react'
import { errorMessage } from '@/http'
import { useToast } from '@/ui/toast'

export function useRun(onDone?: () => void) {
  const [error, setError] = useState<string | null>(null)
  const toast = useToast()

  const run = useCallback(
    async (fn: () => Promise<unknown>) => {
      setError(null)
      try {
        await fn()
        onDone?.()
        return true
      } catch (e) {
        setError(errorMessage(e))
        return false
      }
    },
    [onDone],
  )

  const runAndToast = useCallback(
    (fn: () => Promise<unknown>, message: string) =>
      run(fn).then((ok) => {
        if (ok) toast(message)
        return ok
      }),
    [run, toast],
  )

  return { error, run, runAndToast, setError }
}
