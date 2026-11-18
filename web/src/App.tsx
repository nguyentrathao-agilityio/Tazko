import { useCallback, useEffect, useState } from 'react'
import { Refused } from '@/auth/Refused'
import { SignIn } from '@/auth/SignIn'
import { Home } from '@/Home'

function current() {
  return { path: window.location.pathname, search: window.location.search }
}

export function App() {
  const [loc, setLoc] = useState(current)

  useEffect(() => {
    const onPop = () => setLoc(current())
    window.addEventListener('popstate', onPop)

    return () => window.removeEventListener('popstate', onPop)
  }, [])

  const go = useCallback((to: string) => {
    window.history.pushState(null, '', to)
    setLoc(current())
  }, [])

  if (loc.path === '/signin') return <SignIn search={loc.search} />
  if (loc.path === '/signin/refused') return <Refused />
  return <Home go={go} />
}
