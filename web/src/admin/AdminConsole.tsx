import type { Me } from '@/api'
import { t } from '@/i18n'
import { EmptyState } from '@/ui/EmptyState'
import type { Go } from '@/ui/Link'
import { AdminShell } from '@/admin/AdminShell'
import { Audit } from '@/admin/Audit'
import { Companies } from '@/admin/Companies'
import { CompanyDetail } from '@/admin/CompanyDetail'
import { Invites } from '@/admin/Invites'
import { People } from '@/admin/People'
import { Server } from '@/admin/Server'

export function AdminConsole({ me, path, go, onSignOut }: { me: Me; path: string; go: Go; onSignOut: () => void }) {
  if (!me.is_super_admin) {
    return (
      <main className="page">
        <EmptyState title={t('admin.notAllowed')} />
      </main>
    )
  }

  const company = /^\/admin\/companies\/([a-z0-9-]+)\/?$/.exec(path)?.[1]
  const section = company ? '/admin/companies' : path.replace(/\/$/, '')
  let page
  if (company) page = <CompanyDetail key={company} slug={company} go={go} />
  else if (section === '/admin/people') page = <People me={me.id} />
  else if (section === '/admin/invites') page = <Invites />
  else if (section === '/admin/audit') page = <Audit />
  else if (section === '/admin/server') page = <Server />
  else page = <Companies go={go} />

  return (
    <AdminShell me={me} active={section === '/admin' ? '/admin/companies' : section} go={go} onSignOut={onSignOut}>
      {page}
    </AdminShell>
  )
}
