import { useCallback, useEffect, useRef, useState } from 'react'

export function useLoad<T>(load: () => Promise<T>, key: string) {
  const [state, setState] = useState<{ key: string; data: T | null; error: string | null }>({
    key,
    data: null,
    error: null,
  })
  const [version, setVersion] = useState(0)
  const loader = useRef(load)

  useEffect(() => {
    loader.current = load
  })

  useEffect(() => {
    let live = true
    loader
      .current()
      .then((data) => {
        if (live) setState({ key, data, error: null })
      })
      .catch((e: unknown) => {
        if (live) setState((s) => ({ ...s, key, error: e instanceof Error ? e.message : String(e) }))
      })

    return () => {
      live = false
    }
  }, [key, version])

  const reload = useCallback(() => setVersion((v) => v + 1), [])
  const current = state.key === key
  return { data: current ? state.data : null, error: current ? state.error : null, reload }
}
