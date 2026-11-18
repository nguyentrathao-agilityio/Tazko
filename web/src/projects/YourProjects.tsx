import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { BoardsPicture, EmptyState } from '@/ui/EmptyState'
import { initials } from '@/ui/names'
import type { Go } from '@/ui/Link'
import { PageHead } from '@/ui/PageHead'
import { useLoad } from '@/ui/useLoad'
import { Stack } from '@/projects/Stack'
import type { MyProject } from '@/projects/types'

export function YourProjects({ company, go }: { company: Company; go: Go }) {
  const path = `/api/v1/c/${company.slug}/my-projects`
  const { data, error } = useLoad(() => api<{ projects: MyProject[] }>('GET', path), path)

  return (
    <>
      <PageHead title={t('nav.projects')} lead={t('projects.lead', { company: company.name })}>
        {company.role === 'owner' && (
          <button type="button" className="btn" onClick={() => go(`/c/${company.slug}/projects`)}>
            <Icon name="building" />
            {t('nav.manageProjects')}
          </button>
        )}
      </PageHead>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {!data && !error && (
        <p role="status" className="muted">
          {t('common.loading')}
        </p>
      )}
      {data && data.projects.length === 0 && (
        <EmptyState picture={<BoardsPicture />} title={t('projects.emptyTitle')} body={t('projects.emptyBody', { company: company.name })} />
      )}
      {data && data.projects.length > 0 && (
        <>
          <div className="sechd">
            <Icon name="folder" />
            {t('projects.yours')}
          </div>
          <div className="pgrid">
            {data.projects.map((p) => (
              <button key={p.id} type="button" className="ptile" onClick={() => go(`/c/${company.slug}/p/${p.id}`)}>
                <div className="row" style={{ flexWrap: 'nowrap' }}>
                  <span className={`av sq ${p.color}`} style={{ width: 36, height: 36 }} aria-hidden="true">
                    {initials(p.name)}
                  </span>
                  <h3>{p.name}</h3>
                </div>
                <p>{p.description || t('projects.noDescription')}</p>
                <div className="foot">
                  <Stack people={p.members} />
                  <span>{t('projects.boardCount', { n: p.boards })}</span>
                  <span className={`pill${p.role === 'owner' ? ' accent' : ''}`} style={{ marginLeft: 'auto' }}>
                    {t(`projectRole.${p.role}`)}
                  </span>
                </div>
              </button>
            ))}
          </div>
        </>
      )}
    </>
  )
}
