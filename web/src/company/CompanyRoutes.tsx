import { TemplatesPage } from '@/templates/TemplatesPage'
import { useEffect, type ReactNode } from 'react'
import type { Me } from '@/api'
import { t } from '@/i18n'
import { EmptyState } from '@/ui/EmptyState'
import { Link, type Go } from '@/ui/Link'
import { CompanySettings } from '@/company/CompanySettings'
import { CompanyShell } from '@/company/CompanyShell'
import { lastCompany, rememberCompany } from '@/company/lastCompany'
import { ManageProjects } from '@/company/ManageProjects'
import { MyCardsPage } from '@/mycards/MyCardsPage'
import { IntegrationsPage } from '@/tokens/IntegrationsPage'
import { ProjectPage } from '@/projects/ProjectPage'
import { BoardPage, type BoardView } from '@/board/BoardPage'
import { YourProjects } from '@/projects/YourProjects'
import { SearchHost } from '@/search/Search'
import { SettingsPage } from '@/settings/SettingsPage'
import { MeContext } from '@/ui/meContext'
import { companyRoute } from '@/paths'

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
  const match = /^\/c\/([a-z0-9-]+)(?<section>\/settings|\/projects|\/p\/(?<project>\d+)(?<tab>\/members|\/settings|\/templates)?|\/b\/(?<board>\d+)(?<view>\/table|\/dashboard)?(?:\/c\/(?<card>\d+))?|\/my-cards|\/integrations|\/me(?<me>\/appearance|\/tokens|\/sessions)?)?\/?$/.exec(path)
  const slug = match?.[1]
  const g = match?.groups ?? {}
  const company = me.companies.find((c) => c.slug === slug)
  const fallback = me.companies.find((c) => c.slug === lastCompany()) ?? me.companies[0]

  useEffect(() => {
    if (company) rememberCompany(company.slug)
  }, [company])

  useEffect(() => {
    if (!match && fallback) {
      go(companyRoute(fallback.slug), true)
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

  if (g.board)
    return (
      <MeContext.Provider value={me}>
        <BoardPage key={`${company.slug}:b${g.board}`} company={company} id={Number(g.board)} cardId={g.card ? Number(g.card) : null} me={me.id} go={go} view={(g.view?.slice(1) as BoardView | undefined) ?? 'board'} />
        <SearchHost company={company} go={go} />
      </MeContext.Provider>
    )

  const projectId = g.project ? Number(g.project) : null
  const mine = g.section?.startsWith('/me') ?? false
  const section = mine ? 'me' : g.section === '/my-cards' ? 'mycards' : g.section === '/integrations' ? 'integrations' : projectId ? 'home' : g.section === '/settings' ? 'settings' : g.section === '/projects' ? 'projects' : 'home'
  const tab = g.tab === '/members' ? 'members' : g.tab === '/settings' ? 'settings' : 'boards'
  return (
    <MeContext.Provider value={me}>
    <CompanyShell me={me} company={company} active={section} go={go} onSignOut={onSignOut}>
      {projectId && g.tab === '/templates' && <TemplatesPage key={`${company.slug}:t${projectId}`} company={company} project={projectId} go={go} />}
      {projectId && g.tab !== '/templates' && <ProjectPage key={`${company.slug}:${projectId}`} company={company} id={projectId} tab={tab} go={go} />}
      {!projectId && section === 'home' && <YourProjects key={company.slug} company={company} go={go} />}
      {section === 'me' && (
        <SettingsPage me={me} company={company} tab={g.me === '/appearance' ? 'appearance' : g.me === '/tokens' ? 'tokens' : g.me === '/sessions' ? 'sessions' : 'profile'} go={go} onProfile={onProfile} />
      )}
      {section === 'integrations' && <IntegrationsPage key={company.slug} company={company} />}
      {section === 'mycards' && <MyCardsPage key={company.slug} company={company} go={go} />}
      {section === 'settings' && <CompanySettings key={company.slug} company={company} />}
      {section === 'projects' && <ManageProjects key={company.slug} company={company} />}
      <SearchHost company={company} go={go} />
    </CompanyShell>
    </MeContext.Provider>
  )
}
