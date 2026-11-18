import { useEffect, useState } from 'react'
import { fetchHealth, type Health } from '@/health'
import { t } from '@/i18n'

type State = { kind: 'checking' } | { kind: 'ok'; health: Health } | { kind: 'down' }

export function App() {
  const [state, setState] = useState<State>({ kind: 'checking' })

  useEffect(() => {
    const ctrl = new AbortController()
    fetchHealth(ctrl.signal)
      .then((health) => setState({ kind: 'ok', health }))
      .catch(() => {
        if (!ctrl.signal.aborted) setState({ kind: 'down' })
      })

    return () => ctrl.abort()
  }, [])

  return (
    <main className="mx-auto flex min-h-screen max-w-xl flex-col items-center justify-center gap-3 px-4 text-center">
      <svg width="44" height="44" viewBox="0 0 44 44" aria-hidden="true">
        <rect width="44" height="44" rx="12" fill="var(--accent)" />
        <rect x="10" y="10" width="7" height="24" rx="2.5" fill="#fff" />
        <rect x="19" y="10" width="7" height="15" rx="2.5" fill="#fff" opacity=".85" />
        <rect x="28" y="10" width="7" height="19" rx="2.5" fill="#fff" opacity=".7" />
      </svg>
      <h1 className="text-2xl font-bold">{t('app.name')}</h1>
      <p className="text-muted">{t('app.tagline')}</p>
      <p role="status" className="text-sm text-muted">
        {state.kind === 'checking' && t('app.status.checking')}
        {state.kind === 'ok' && t('app.status.ok', { version: state.health.version })}
        {state.kind === 'down' && t('app.status.down')}
      </p>
    </main>
  )
}
