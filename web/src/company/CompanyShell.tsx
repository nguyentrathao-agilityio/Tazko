import { useEffect, useRef, useState, type ReactNode } from 'react'
import type { Company, Me } from '@/api'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { Link, type Go } from '@/ui/Link'
import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import { useToast } from '@/ui/toast'
import { companyRoleLabel, isOwner } from '@/roles'
import { ESCAPE } from '@/keys/keyNames'
import { companyRoute } from '@/paths'
import { Palette } from '@/palette/Palette'
import { usePalette, usePersonal } from '@/palette/usePalette'
import { personalCommands } from '@/palette/commands'

function Switcher({ me, company, go }: { me: Me; company: Company; go: Go }) {
  const [open, setOpen] = useState(false)
  const toast = useToast()
  const box = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const onDown = (e: MouseEvent) => {
      if (!box.current?.contains(e.target as Node)) setOpen(false)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === ESCAPE) setOpen(false)
    }
    document.addEventListener('mousedown', onDown)
    document.addEventListener('keydown', onKey)

    return () => {
      document.removeEventListener('mousedown', onDown)
      document.removeEventListener('keydown', onKey)
    }
  }, [open])

  const pick = (to: string, name?: string) => {
    setOpen(false)
    go(to)
    if (name) toast(t('toast.switched', { company: name }))
  }

  return (
    <div className="switch" ref={box}>
      <button type="button" aria-expanded={open} aria-haspopup="menu" aria-label={t('switcher.label')} onClick={() => setOpen((o) => !o)}>
        <Avatar name={company.name} square />
        <span style={{ flex: 1, minWidth: 0 }}>
          <b>{company.name}</b>
          <small>{t(companyRoleLabel(company.role))}</small>
        </span>
        <Icon name="chev" />
      </button>
      {open && (
        <div className="menu" role="menu">
          {me.companies.map((c) => (
            <button key={c.slug} type="button" role="menuitem" className="item" onClick={() => pick(companyRoute(c.slug), c.slug === company.slug ? undefined : c.name)}>
              <Avatar name={c.name} square />
              {c.name}
              {c.slug === company.slug && (
                <span style={{ marginLeft: 'auto' }}>
                  <Icon name="check" />
                </span>
              )}
            </button>
          ))}
          {me.is_super_admin && (
            <>
              <hr />
              <button type="button" role="menuitem" className="item" onClick={() => pick('/admin/companies')}>
                <Icon name="shield" />
                {t('switcher.adminConsole')}
              </button>
            </>
          )}
        </div>
      )}
    </div>
  )
}

export function CompanyShell({
  me,
  company,
  active,
  go,
  onSignOut,
  children,
}: {
  me: Me
  company: Company
  active: 'home' | 'mycards' | 'projects' | 'settings' | 'me'
  go: Go
  onSignOut: () => void
  children: ReactNode
}) {
  const name = displayName(me.name, me.email)
  const base = companyRoute(company.slug)
  const owner = isOwner(company.role)
  const palette = usePalette()
  const personal = usePersonal(go, base)

  return (
    <div className="shell">
      <aside className="rail">
        <Switcher me={me} company={company} go={go} />
        <Link to={base} go={go} className={`nav${active === 'home' ? ' on' : ''}`}>
          <Icon name="grid" />
          {t('nav.projects')}
        </Link>
        <Link to={`${base}/my-cards`} go={go} className={`nav${active === 'mycards' ? ' on' : ''}`}>
          <Icon name="checksq" />
          {t('nav.myCards')}
        </Link>
        {owner && (
          <>
            <Link to={`${base}/projects`} go={go} className={`nav${active === 'projects' ? ' on' : ''}`}>
              <Icon name="building" />
              {t('nav.manageProjects')}
            </Link>
            <Link to={`${base}/settings`} go={go} className={`nav${active === 'settings' ? ' on' : ''}`}>
              <Icon name="gear" />
              {t('nav.companySettings')}
            </Link>
          </>
        )}
        <div className="grow" />
        <Link to={`${base}/me`} go={go} className={`nav me${active === 'me' ? ' on' : ''}`}>
          <Avatar name={name} />
          <div>
            <b>{name}</b>
            <small>
              {me.is_super_admin ? t('rail.superAdmin') : t('nav.member')} · {t('settings.title')}
            </small>
          </div>
        </Link>
        {palette.open && (
          <Palette
            items={personalCommands(personal)}
            onClose={palette.close}
          />
        )}
        <button type="button" className="nav" onClick={onSignOut}>
          <Icon name="out" />
          {t('home.signOut')}
        </button>
      </aside>
      <main className="page">{children}</main>
    </div>
  )
}
