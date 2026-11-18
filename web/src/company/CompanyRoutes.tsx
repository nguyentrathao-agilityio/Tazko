import { useEffect, type ReactNode } from 'react'
import type { Me } from '@/api'
import { t } from '@/i18n'
import { EmptyState } from '@/ui/EmptyState'
import { Link, type Go } from '@/ui/Link'
import { CompanySettings } from '@/company/CompanySettings'
import { CompanyShell } from '@/company/CompanyShell'
import { lastCompany, rememberCompany } from '@/company/lastCompany'
import { ManageProjects } from '@/company/ManageProjects'
import { ProjectPage } from '@/projects/ProjectPage'
import { BoardPage } from '@/board/BoardPage'
import { YourProjects } from '@/projects/YourProjects'
import { SearchHost } from '@/search/Search'
import { SettingsPage } from '@/settings/SettingsPage'

function Centered({ title, body, children }: { title: string; body: string; children?: ReactNode }) {
  return (
    <main className="page mx-auto">
      <EmptyState title={title} body={body}>
        {children}
      </EmptyState>
    </main>
  )
}

export function CompanyRoutes({ me, path, go, onSignOut, onProfile }: { me: Me; path: string; go: Go; onSignOut: () => void; onProfile: () => void }) {
  const match = /^\/c\/([a-z0-9-]+)(\/settings|\/projects|\/p\/(\d+)(\/members|\/settings)?|\/b\/(\d+)(?:\/c\/(\d+))?|\/me(\/appearance|\/sessions)?)?\/?$/.exec(path)
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
        {me.is_super_admin && (
          <Link to="/admin/companies" go={go} className="btn primary">
            {t('switcher.adminConsole')}
          </Link>
        )}
        <button type="button" className="btn" onClick={onSignOut}>
          {t('home.signOut')}
        </button>
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

  if (match[5])
    return (
      <>
        <BoardPage key={`${company.slug}:b${match[5]}`} company={company} id={Number(match[5])} cardId={match[6] ? Number(match[6]) : null} me={me.id} go={go} />
        <SearchHost company={company} go={go} />
      </>
    )

  const projectId = match[3] ? Number(match[3]) : null
  const mine = match[2]?.startsWith('/me') ?? false
  const section = mine ? 'me' : projectId ? 'home' : match[2] === '/settings' ? 'settings' : match[2] === '/projects' ? 'projects' : 'home'
  const tab = match[4] === '/members' ? 'members' : match[4] === '/settings' ? 'settings' : 'boards'
  return (
    <CompanyShell me={me} company={company} active={section} go={go} onSignOut={onSignOut}>
      {projectId && <ProjectPage key={`${company.slug}:${projectId}`} company={company} id={projectId} tab={tab} go={go} />}
      {!projectId && section === 'home' && <YourProjects key={company.slug} company={company} go={go} />}
      {section === 'me' && (
        <SettingsPage me={me} company={company} tab={match[7] === '/appearance' ? 'appearance' : match[7] === '/sessions' ? 'sessions' : 'profile'} go={go} onProfile={onProfile} />
      )}
      {section === 'settings' && <CompanySettings key={company.slug} company={company} />}
      {section === 'projects' && <ManageProjects key={company.slug} company={company} />}
      <SearchHost company={company} go={go} />
    </CompanyShell>
  )
}
