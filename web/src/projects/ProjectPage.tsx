import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { Link, type Go } from '@/ui/Link'
import { initials } from '@/ui/names'
import { EmptyState } from '@/ui/EmptyState'
import { useLoad } from '@/ui/useLoad'
import { Tools } from '@/ui/Tools'
import { ProjectBoards } from '@/projects/ProjectBoards'
import { ProjectMembers } from '@/projects/ProjectMembers'
import { ProjectSettings } from '@/projects/ProjectSettings'
import type { ProjectView } from '@/projects/types'
import { isOwner } from '@/roles'
import { companyRoute, projectApi } from '@/paths'

export type ProjectTab = 'boards' | 'members' | 'settings'

export function RoleChip({ role }: { role: ProjectView['role'] }) {
  return <span className={`pill${isOwner(role) ? ' accent' : ''}`}>{t(`projectRole.chip.${role}`)}</span>
}

export function ProjectPage({ company, id, tab, go }: { company: Company; id: number; tab: ProjectTab; go: Go }) {
  const path = projectApi(company.slug, id)
  const home = companyRoute(company.slug)
  const { data, error, reload } = useLoad(() => api<ProjectView>('GET', path), path)
  const base = `${home}/p/${id}`

  if (error)
    return (
      <EmptyState title={t('project.missingTitle')} body={t('project.missingBody')}>
        <Link to={home} go={go} className="btn">
          {t('project.back')}
        </Link>
      </EmptyState>
    )
  if (!data)
    return (
      <p role="status" className="muted">
        {t('common.loading')}
      </p>
    )

  const shown: ProjectTab = tab === 'settings' && !data.can.manage ? 'boards' : tab
  return (
    <>
      <div className="crumb">
        <Link to={home} go={go}>
          {t('nav.projects')}
        </Link>{' '}
        / {data.name}
      </div>
      <div className="head">
        <div className="who">
          <span className={`av sq ${data.color}`} style={{ width: 46, height: 46, fontSize: '1rem' }} aria-hidden="true">
            {initials(data.name)}
          </span>
          <div>
            <h1>{data.name}</h1>
            <p className="muted" style={{ margin: 0 }}>
              {data.description || t('project.noDescription')}
            </p>
          </div>
        </div>
        <RoleChip role={data.role} />
        <span className="sp" />
        <Tools />
      </div>
      <div className="tabs2" role="tablist">
        <button type="button" role="tab" aria-selected={shown === 'boards'} className={shown === 'boards' ? 'on' : ''} onClick={() => go(base)}>
          {t('project.tab.boards')}
        </button>
        <button type="button" role="tab" aria-selected={shown === 'members'} className={shown === 'members' ? 'on' : ''} onClick={() => go(`${base}/members`)}>
          {t('project.tab.members', { n: data.member_count })}
        </button>
        {data.can.manage && (
          <button type="button" role="tab" aria-selected={shown === 'settings'} className={shown === 'settings' ? 'on' : ''} onClick={() => go(`${base}/settings`)}>
            {t('project.tab.settings')}
          </button>
        )}
      </div>
      {shown === 'boards' && <ProjectBoards company={company} project={data} go={go} />}
      {shown === 'members' && <ProjectMembers company={company} project={data} onChange={reload} />}
      {shown === 'settings' && <ProjectSettings company={company} project={data} onSaved={reload} />}
    </>
  )
}
