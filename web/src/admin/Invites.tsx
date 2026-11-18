import { useState } from 'react'
import { api, HttpError } from '@/http'
import { t } from '@/i18n'
import { EmptyState } from '@/ui/EmptyState'
import { PageHead } from '@/ui/PageHead'
import { useToast } from '@/ui/toast'
import { useLoad } from '@/ui/useLoad'
import { InviteTable } from '@/admin/MembersPanel'
import type { Invite } from '@/admin/types'

export function Invites() {
  const { data, error, reload } = useLoad(() => api<{ invites: Invite[] }>('GET', '/api/v1/admin/invites'), 'invites')
  const [problem, setProblem] = useState<string | null>(null)
  const toast = useToast()

  const attempt = async (fn: () => Promise<unknown>) => {
    setProblem(null)
    try {
      await fn()
      reload()
      return true
    } catch (e) {
      setProblem(e instanceof HttpError ? e.message : String(e))
      return false
    }
  }

  const revoke = async (id: number) => {
    const v = data?.invites.find((i) => i.id === id)
    if (!v || !(await attempt(() => api('DELETE', `/api/v1/admin/invites/${id}`)))) return
    toast(t('toast.revoked', { email: v.email }), () =>
      attempt(() => api('POST', `/api/v1/admin/companies/${v.company_slug}/invites`, { email: v.email, role: v.role })),
    )
  }

  return (
    <>
      <PageHead title={t('rail.invites')} lead={t('invites.lead')} />
      {(error || problem) && (
        <p className="error" role="alert">
          {error ?? problem}
        </p>
      )}
      {data && data.invites.length === 0 ? (
        <EmptyState title={t('invites.emptyTitle')} />
      ) : (
        <InviteTable invites={data?.invites ?? []} withCompany onRevoke={(id) => void revoke(id)} />
      )}
    </>
  )
}
