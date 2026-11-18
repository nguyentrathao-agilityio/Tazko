import type { Company } from '@/api'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { BoardsPicture, EmptyState } from '@/ui/EmptyState'
import type { Go } from '@/ui/Link'
import { PageHead } from '@/ui/PageHead'

export function CompanyHome({ company, go }: { company: Company; go: Go }) {
  return (
    <>
      <PageHead title={t('nav.projects')} lead={t('companyHome.lead', { company: company.name })}>
        {company.role === 'owner' && (
          <button type="button" className="btn primary" onClick={() => go(`/c/${company.slug}/projects`)}>
            <Icon name="plus" />
            {t('companyHome.newProject')}
          </button>
        )}
      </PageHead>
      <EmptyState picture={<BoardsPicture />} title={t('companyHome.emptyTitle')} body={t('companyHome.emptyBody')} />
    </>
  )
}

export function NoAccess({ company }: { company: Company }) {
  return <EmptyState title={t('noAccess.title')} body={t('noAccess.body', { company: company.name })} />
}
