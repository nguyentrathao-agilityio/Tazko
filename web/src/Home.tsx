import type { Me } from '@/api'
import { Button } from '@/components/ui/button'
import { t } from '@/i18n'
import { Link, type Go } from '@/ui/Link'
import { Mark } from '@/ui/Mark'

export function Home({ me, go, onSignOut }: { me: Me; go: Go; onSignOut: () => void }) {
  return (
    <main className="mx-auto max-w-2xl px-4 py-10">
      <header className="flex items-center gap-3">
        <Mark size={32} />
        <h1 className="flex-1 text-xl font-bold">{t('app.name')}</h1>
        {me.is_super_admin && (
          <Link to="/admin/companies" go={go} className="btn sm">
            {t('rail.console')}
          </Link>
        )}
        <Button variant="secondary" size="sm" onClick={onSignOut}>
          {t('home.signOut')}
        </Button>
      </header>
      <p className="mt-6 text-muted">{t('home.signedInAs', { email: me.email })}</p>
      <h2 className="mt-6 text-lg font-semibold">{t('home.companies')}</h2>
      {me.companies.length === 0 ? (
        <p className="mt-2 text-muted">{t('home.noCompanies')}</p>
      ) : (
        <ul className="mt-2 divide-y divide-line rounded-xl border border-line bg-surface">
          {me.companies.map((c) => (
            <li key={c.slug} className="flex items-center justify-between px-4 py-3">
              <span className="font-semibold">{c.name}</span>
              <span className="text-sm text-muted">{t(c.role === 'owner' ? 'role.owner' : 'role.member')}</span>
            </li>
          ))}
        </ul>
      )}
    </main>
  )
}
