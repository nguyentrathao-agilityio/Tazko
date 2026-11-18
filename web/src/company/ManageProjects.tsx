import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { useLoad } from '@/ui/useLoad'
import { NoAccess } from '@/company/CompanyHome'

export function ManageProjects({ company }: { company: Company }) {
  if (company.role !== 'owner') return <NoAccess company={company} />
  return <ProjectList company={company} />
}

function ProjectList({ company }: { company: Company }) {
  const path = `/api/v1/c/${company.slug}/projects`
  const { data, error } = useLoad(() => api<{ projects: { name: string }[] }>('GET', path), path)

  return (
    <>
      <div className="head">
        <div>
          <h1>{t('nav.manageProjects')}</h1>
          <p className="muted">{t('manage.lead', { company: company.name })}</p>
        </div>
        <span className="sp" />
        <button type="button" className="btn primary" disabled title={t('manage.soon')}>
          <Icon name="plus" />
          {t('companyHome.newProject')}
        </button>
      </div>
      <div className="note" style={{ margin: '0 0 16px' }}>
        {t('manage.note')}
      </div>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <div className="tw">
        <table>
          <thead>
            <tr>
              <th>{t('projects.col.project')}</th>
              <th>{t('projects.col.owners')}</th>
              <th className="num">{t('companies.col.members')}</th>
              <th className="num">{t('companies.col.boards')}</th>
              <th>{t('manage.col.status')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {data && data.projects.length === 0 && (
              <tr>
                <td colSpan={6} className="muted">
                  {t('manage.empty')} {t('manage.soon')}
                </td>
              </tr>
            )}
            {data?.projects.map((p) => (
              <tr key={p.name}>
                <td>
                  <b>{p.name}</b>
                </td>
                <td />
                <td className="num" />
                <td className="num" />
                <td />
                <td />
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  )
}
