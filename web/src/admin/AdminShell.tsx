import type { ReactNode } from 'react'
import type { Me } from '@/api'
import { t, type MessageKey } from '@/i18n'
import { Icon, type IconName } from '@/ui/Icon'
import { Link, type Go } from '@/ui/Link'
import { Avatar} from '@/ui/people'
import { displayName } from '@/ui/names'

const sections: [string, IconName, MessageKey][] = [
  ['/admin/companies', 'building', 'rail.companies'],
  ['/admin/people', 'users', 'rail.people'],
  ['/admin/invites', 'mail', 'rail.invites'],
  ['/admin/audit', 'log', 'rail.audit'],
  ['/admin/server', 'server', 'rail.server'],
]

export function AdminShell({
  me,
  active,
  go,
  onSignOut,
  children,
}: {
  me: Me
  active: string
  go: Go
  onSignOut: () => void
  children: ReactNode
}) {
  const name = displayName(me.name, me.email)
  return (
    <div className="shell">
      <aside className="rail">
        <Link to="/" go={go} className="nav">
          <Icon name="back" />
          {t('rail.backToApp')}
        </Link>
        <h4>{t('rail.console')}</h4>
        <nav aria-label={t('rail.console')} style={{ display: 'contents' }}>
          {sections.map(([to, icon, label]) => (
            <Link key={to} to={to} go={go} className={`nav${active === to ? ' on' : ''}`}>
              <Icon name={icon} />
              {t(label)}
            </Link>
          ))}
        </nav>
        <div className="grow" />
        <div className="nav me">
          <Avatar name={name} />
          <div>
            <b>{name}</b>
            <small>{t('rail.superAdmin')}</small>
          </div>
        </div>
        <button type="button" className="nav" onClick={onSignOut}>
          <Icon name="out" />
          {t('home.signOut')}
        </button>
      </aside>
      <main className="page">{children}</main>
    </div>
  )
}
