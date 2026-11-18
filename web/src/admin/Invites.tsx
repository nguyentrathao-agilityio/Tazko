import { useState } from 'react'
import { api, HttpError } from '@/http'
import { t } from '@/i18n'
import { useLoad } from '@/ui/useLoad'
import { InviteTable } from '@/admin/MembersPanel'
import type { Invite } from '@/admin/types'

export function Invites() {
  const { data, error, reload } = useLoad(() => api<{ invites: Invite[] }>('GET', '/api/v1/admin/invites'), 'invites')
  const [problem, setProblem] = useState<string | null>(null)

  const revoke = async (id: number) => {
    setProblem(null)
    try {
      await api('DELETE', `/api/v1/admin/invites/${id}`)
      reload()
    } catch (e) {
      setProblem(e instanceof HttpError ? e.message : String(e))
    }
  }

  return (
    <>
      <div className="head">
        <div>
          <h1>{t('rail.invites')}</h1>
          <p className="muted">{t('invites.lead')}</p>
        </div>
      </div>
      {(error || problem) && (
        <p className="error" role="alert">
          {error ?? problem}
        </p>
      )}
      {data && data.invites.length === 0 ? (
        <div className="empty">
          <h2>{t('invites.emptyTitle')}</h2>
        </div>
      ) : (
        <InviteTable invites={data?.invites ?? []} withCompany onRevoke={(id) => void revoke(id)} />
      )}
    </>
  )
}
