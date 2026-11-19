import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { useLoad } from '@/ui/useLoad'
import { Storage } from '@/admin/Companies'
import { MembersPanel } from '@/admin/MembersPanel'
import type { Invite, Member } from '@/admin/types'
import { NoAccess } from '@/company/CompanyHome'
import { isOwner } from '@/roles'
import { ErrorNote } from '@/ui/ErrorNote'
import { companyApi } from '@/paths'

type Settings = {
  members: Member[]
  invites: Invite[]
  storage_used: number
  storage_limit: number | null
}

export function CompanySettings({ company }: { company: Company }) {
  if (!isOwner(company.role)) return <NoAccess company={company} />
  return <OwnerSettings company={company} />
}

function OwnerSettings({ company }: { company: Company }) {
  const base = companyApi(company.slug)
  const { data, error, reload } = useLoad(() => api<Settings>('GET', `${base}/settings`), base)

  return (
    <>
      <div className="head">
        <div>
          <h1>{t('nav.companySettings')}</h1>
          <p className="muted">{t('companySettings.lead', { company: company.name })}</p>
        </div>
      </div>
      <ErrorNote message={error} />
      {data && (
        <>
          <MembersPanel
            base={base}
            revokePath={(id) => `${base}/invites/${id}`}
            members={data.members}
            invites={data.invites}
            onChange={reload}
          />
          <section className="box">
            <h2>{t('companySettings.storage')}</h2>
            <Storage used={data.storage_used} limit={data.storage_limit} />
            <p className="hint" style={{ marginTop: 10 }}>
              {t('companySettings.storageHint')}
            </p>
          </section>
        </>
      )}
    </>
  )
}
