import { useEffect, type ReactNode } from 'react'
import type { Me } from '@/api'
import { t } from '@/i18n'
import { Link, type Go } from '@/ui/Link'
import { CompanyHome } from '@/company/CompanyHome'
import { CompanySettings } from '@/company/CompanySettings'
import { CompanyShell } from '@/company/CompanyShell'
import { lastCompany, rememberCompany } from '@/company/lastCompany'
import { ManageProjects } from '@/company/ManageProjects'

function Centered({ title, body, children }: { title: string; body: string; children?: ReactNode }) {
  return (
    <main className="page" style={{ margin: '0 auto' }}>
      <div className="empty">
        <h2>{title}</h2>
        <p className="muted" style={{ margin: '6px auto 0', maxWidth: '48ch' }}>
          {body}
        </p>
        {children && <div style={{ marginTop: 16 }}>{children}</div>}
      </div>
    </main>
  )
}

export function CompanyRoutes({ me, path, go, onSignOut }: { me: Me; path: string; go: Go; onSignOut: () => void }) {
  const match = /^\/c\/([a-z0-9-]+)(\/settings|\/projects)?\/?$/.exec(path)
  const slug = match?.[1]
  const company = me.companies.find((c) => c.slug === slug)
  const fallback = me.companies.find((c) => c.slug === lastCompany()) ?? me.companies[0]

  useEffect(() => {
    if (company) rememberCompany(company.slug)
  }, [company])

  useEffect(() => {
    if (!match && fallback) {
      go(`/c/${fallback.slug}`, true)
    }
  }, [match, fallback, go])

  if (!match && !fallback) {
    return (
      <Centered title={t('noCompany.title')} body={t('noCompany.body')}>
        <div className="row" style={{ justifyContent: 'center' }}>
          {me.is_super_admin && (
            <Link to="/admin/companies" go={go} className="btn primary">
              {t('switcher.adminConsole')}
            </Link>
          )}
          <button type="button" className="btn" onClick={onSignOut}>
            {t('home.signOut')}
          </button>
        </div>
      </Centered>
    )
  }

  if (!match) return null

  if (!company) {
    return (
      <Centered title={t('missingCompany.title')} body={t('missingCompany.body')}>
        <Link to="/" go={go} className="btn">
          {t('missingCompany.back')}
        </Link>
      </Centered>
    )
  }

  const section = match[2] === '/settings' ? 'settings' : match[2] === '/projects' ? 'projects' : 'home'
  return (
    <CompanyShell me={me} company={company} active={section} go={go} onSignOut={onSignOut}>
      {section === 'home' && <CompanyHome key={company.slug} company={company} go={go} />}
      {section === 'settings' && <CompanySettings key={company.slug} company={company} />}
      {section === 'projects' && <ManageProjects key={company.slug} company={company} />}
    </CompanyShell>
  )
}
