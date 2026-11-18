import type { Company } from '@/api'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import type { Go } from '@/ui/Link'

export function CompanyHome({ company, go }: { company: Company; go: Go }) {
  return (
    <>
      <div className="head">
        <div>
          <h1>{t('nav.projects')}</h1>
          <p className="muted">{t('companyHome.lead', { company: company.name })}</p>
        </div>
        <span className="sp" />
        {company.role === 'owner' && (
          <button type="button" className="btn primary" onClick={() => go(`/c/${company.slug}/projects`)}>
            <Icon name="plus" />
            {t('companyHome.newProject')}
          </button>
        )}
      </div>
      <div className="empty">
        <svg width="120" height="80" viewBox="0 0 120 80" aria-hidden="true" style={{ margin: '0 auto 14px', display: 'block' }}>
          <rect x="6" y="8" width="30" height="64" rx="8" fill="var(--accent-soft)" />
          <rect x="45" y="8" width="30" height="42" rx="8" fill="var(--accent-line)" />
          <rect x="84" y="8" width="30" height="54" rx="8" fill="var(--accent-soft)" />
        </svg>
        <h2>{t('companyHome.emptyTitle')}</h2>
        <p className="muted" style={{ maxWidth: '48ch', margin: '6px auto 0' }}>
          {t('companyHome.emptyBody')}
        </p>
      </div>
    </>
  )
}

export function NoAccess({ company }: { company: Company }) {
  return (
    <div className="empty">
      <h2>{t('noAccess.title')}</h2>
      <p className="muted">{t('noAccess.body', { company: company.name })}</p>
    </div>
  )
}
