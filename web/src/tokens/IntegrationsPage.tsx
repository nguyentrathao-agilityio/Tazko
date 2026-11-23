import { useState } from 'react'
import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { companyApi } from '@/paths'
import type { MyProject } from '@/projects/types'
import { PageHead } from '@/ui/PageHead'
import { EmptyState } from '@/ui/EmptyState'
import { ErrorNote } from '@/ui/ErrorNote'
import { isOwner } from '@/roles'
import { Tools } from '@/ui/Tools'
import { useLoad } from '@/ui/useLoad'
import { Terminal } from '@/tokens/Terminal'
import { TokenTable } from '@/tokens/TokenTable'
import { WebhookList } from '@/webhooks/WebhookList'

export function IntegrationsPage({ company }: { company: Company }) {
  const base = companyApi(company.slug)
  const owner = isOwner(company.role)
  const [tab, setTab] = useState<'tokens' | 'hooks'>('tokens')
  const { data, error } = useLoad(() => (owner ? api<{ projects: MyProject[] }>('GET', `${base}/my-projects`) : Promise.resolve({ projects: [] })), `${base}/my-projects`)
  const projects = (data?.projects ?? []).map((p) => ({ id: p.id, name: p.name }))
  if (!owner) return <EmptyState title={t('integrations.ownersOnly')} body={t('integrations.ownersOnlyBody')} />

  return (
    <>
      <PageHead title={t('integrations.title')} lead={t('integrations.lead', { company: company.name })}>
        <Tools />
      </PageHead>
      <div className="tabs2" role="tablist">
        <button type="button" role="tab" aria-selected={tab === 'tokens'} className={tab === 'tokens' ? 'on' : ''} onClick={() => setTab('tokens')}>
          {t('integrations.tokens')}
        </button>
        <button type="button" role="tab" aria-selected={tab === 'hooks'} className={tab === 'hooks' ? 'on' : ''} onClick={() => setTab('hooks')}>
          {t('integrations.webhooks')}
        </button>
      </div>
      <ErrorNote message={error} />
      {data && tab === 'hooks' && <WebhookList path={`${base}/webhooks`} projects={projects} />}
      {data && tab === 'tokens' && (
        <>
          <TokenTable path={`${base}/tokens`} title={t('integrations.tokensTitle')} lead={t('integrations.tokensLead', { company: company.name })} projects={projects} />
          <section className="box">
            <h2>{t('integrations.useToken')}</h2>
            <Terminal text={`curl -H "Authorization: Bearer $TAZKO_TOKEN" \\\n  ${window.location.origin}${base}/my-projects`} label={t('integrations.useToken')} />
            <p className="hint">
              {t('integrations.reference')} <a href="/api/docs">/api/docs</a>
            </p>
          </section>
        </>
      )}
    </>
  )
}
