import { api } from '@/http'
import { t } from '@/i18n'
import { EmptyState } from '@/ui/EmptyState'
import { ErrorNote } from '@/ui/ErrorNote'
import { PageHead } from '@/ui/PageHead'
import { useToast } from '@/ui/toast'
import { useLoad } from '@/ui/useLoad'
import { InviteTable } from '@/admin/MembersPanel'
import type { Invite } from '@/admin/types'
import { useRun } from '@/projects/useRun'
import { ADMIN_API } from '@/paths'

export function Invites() {
  const { data, error, reload } = useLoad(() => api<{ invites: Invite[] }>('GET', `${ADMIN_API}/invites`), 'invites')
  const { error: problem, run } = useRun(reload)
  const toast = useToast()

  const revoke = async (v: Invite) => {
    if (!(await run(() => api('DELETE', `${ADMIN_API}/invites/${v.id}`)))) return
    toast(t('toast.revoked', { email: v.email }), () =>
      run(() => api('POST', `${ADMIN_API}/companies/${v.company_slug}/invites`, { email: v.email, role: v.role })),
    )
  }

  return (
    <>
      <PageHead title={t('rail.invites')} lead={t('invites.lead')} />
      <ErrorNote message={error ?? problem} />
      {data && data.invites.length === 0 ? (
        <EmptyState title={t('invites.emptyTitle')} />
      ) : (
        <InviteTable invites={data?.invites ?? []} withCompany onRevoke={(v) => void revoke(v)} />
      )}
    </>
  )
}
