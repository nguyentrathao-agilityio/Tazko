import { useCallback, useEffect, useState } from 'react'
import { getMe, signOut, SignInNeeded, type Me } from '@/api'
import { AdminConsole } from '@/admin/AdminConsole'
import { CliAuthorize } from '@/cli/CliAuthorize'
import { CLI_AUTHORIZE } from '@/paths'
import { Refused } from '@/auth/Refused'
import { SignIn } from '@/auth/SignIn'
import { CompanyRoutes } from '@/company/CompanyRoutes'
import { t } from '@/i18n'
import { useApplySettings } from '@/settings/apply'

const SIGN_IN = '/signin'
const REFUSED = '/signin/refused'

function current() {
  return { path: window.location.pathname, search: window.location.search }
}

type Session = { kind: 'loading' } | { kind: 'down' } | { kind: 'in'; me: Me }

export function App() {
  const [loc, setLoc] = useState(current)
  const [session, setSession] = useState<Session>({ kind: 'loading' })
  useApplySettings()
  const publicPage = loc.path === SIGN_IN || loc.path === REFUSED

  useEffect(() => {
    const onPop = () => setLoc(current())
    window.addEventListener('popstate', onPop)

    return () => window.removeEventListener('popstate', onPop)
  }, [])

  const go = useCallback((to: string, replace = false) => {
    if (replace) window.history.replaceState(null, '', to)
    else window.history.pushState(null, '', to)
    setLoc(current())
    window.scrollTo?.(0, 0)
  }, [])

  useEffect(() => {
    if (publicPage || session.kind === 'in') return
    const ctrl = new AbortController()
    getMe(ctrl.signal)
      .then((me) => setSession({ kind: 'in', me }))
      .catch((e: unknown) => {
        if (ctrl.signal.aborted) return
        if (e instanceof SignInNeeded) {
          const here = window.location.pathname + window.location.search
          go(here === '/' ? SIGN_IN : `${SIGN_IN}?next=${encodeURIComponent(here)}`)
          return
        }

        setSession({ kind: 'down' })
      })

    return () => ctrl.abort()
  }, [publicPage, session.kind, go])

  const onSignOut = useCallback(async () => {
    if (session.kind === 'in') await signOut(session.me.csrf_token).catch(() => undefined)
    setSession({ kind: 'loading' })
    go(SIGN_IN)
  }, [session, go])

  if (loc.path === SIGN_IN) return <SignIn search={loc.search} />
  if (loc.path === REFUSED) return <Refused />
  if (session.kind === 'down') return <p className="p-8 text-center text-muted">{t('app.status.down')}</p>
  if (session.kind === 'loading') return <p role="status" className="p-8 text-center text-muted">{t('app.status.checking')}</p>
  if (loc.path === CLI_AUTHORIZE) return <CliAuthorize go={go} />
  if (loc.path === '/admin' || loc.path.startsWith('/admin/'))
    return <AdminConsole me={session.me} path={loc.path} go={go} onSignOut={() => void onSignOut()} />
  return <CompanyRoutes me={session.me} path={loc.path} go={go} onSignOut={() => void onSignOut()} onProfile={() => setSession({ kind: 'loading' })} />
}
