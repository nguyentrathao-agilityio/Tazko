import { useState, type FormEvent } from 'react'
import { api, errorMessage } from '@/http'
import { t } from '@/i18n'
import { bytes, when } from '@/ui/format'
import { Icon } from '@/ui/Icon'
import type { Go } from '@/ui/Link'
import { Modal } from '@/ui/Modal'
import { Avatar } from '@/ui/people'
import { EmptyState } from '@/ui/EmptyState'
import { ErrorNote } from '@/ui/ErrorNote'
import { PageHead } from '@/ui/PageHead'
import { useToast } from '@/ui/toast'
import { useLoad } from '@/ui/useLoad'
import type { CompanyRow } from '@/admin/types'
import { ADMIN_API } from '@/paths'

export function StatusPill({ status }: { status: string }) {
  return status === 'active' ? (
    <span className="pill ok">{t('status.active')}</span>
  ) : (
    <span className="pill warn">{t('status.suspended')}</span>
  )
}

export function Storage({ used, limit }: { used: number; limit: number | null }) {
  const pc = limit ? Math.min(100, Math.round((used / limit) * 100)) : 0
  return (
    <>
      <div className="bar" title={`${bytes(used)} of ${bytes(limit)}`}>
        <span style={{ width: `${pc}%` }} />
      </div>
      <small className="muted">
        {bytes(used)} / {bytes(limit)}
      </small>
    </>
  )
}

export function Companies({ go }: { go: Go }) {
  const { data, error, reload } = useLoad(() => api<{ companies: CompanyRow[] }>('GET', `${ADMIN_API}/companies`), 'companies')
  const [creating, setCreating] = useState(false)
  const toast = useToast()

  return (
    <>
      <PageHead title={t('rail.companies')} lead={t('companies.lead')}>
        <button type="button" className="btn primary" onClick={() => setCreating(true)}>
          <Icon name="plus" />
          {t('companies.new')}
        </button>
      </PageHead>
      <ErrorNote message={error} />
      {data && data.companies.length === 0 ? (
        <EmptyState title={t('companies.empty')} />
      ) : (
        <div className="tw">
          <table>
            <thead>
              <tr>
                <th>{t('companies.col.company')}</th>
                <th>{t('companies.col.status')}</th>
                <th className="num">{t('companies.col.members')}</th>
                <th className="num">{t('companies.col.boards')}</th>
                <th>{t('companies.col.storage')}</th>
                <th>{t('companies.col.last')}</th>
              </tr>
            </thead>
            <tbody>
              {(data?.companies ?? []).map((c) => (
                <tr key={c.slug} className="click" onClick={() => go(`/admin/companies/${c.slug}`)}>
                  <td>
                    <div className="who">
                      <Avatar name={c.name} square />
                      <div>
                        <b>
                          <a
                            href={`/admin/companies/${c.slug}`}
                            onClick={(e) => e.preventDefault()}
                            style={{ color: 'inherit', textDecoration: 'none' }}
                          >
                            {c.name}
                          </a>
                        </b>
                        <small className="mono">{c.slug}</small>
                      </div>
                    </div>
                  </td>
                  <td>
                    <StatusPill status={c.status} />
                  </td>
                  <td className="num">{c.members}</td>
                  <td className="num">{c.boards}</td>
                  <td>
                    <Storage used={c.storage_used} limit={c.storage_limit} />
                  </td>
                  <td className="muted">{when(c.last_activity)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {creating && (
        <NewCompany
          onClose={() => setCreating(false)}
          onCreated={(slug, name) => {
            setCreating(false)
            toast(t('toast.companyCreated', { company: name }))
            reload()
            go(`/admin/companies/${slug}`)
          }}
        />
      )}
    </>
  )
}

function NewCompany({ onClose, onCreated }: { onClose: () => void; onCreated: (slug: string, name: string) => void }) {
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const submit = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const f = new FormData(e.currentTarget)
    setBusy(true)
    try {
      const res = await api<{ slug: string; name: string }>('POST', `${ADMIN_API}/companies`, {
        name: String(f.get('name') ?? ''),
        slug: String(f.get('slug') ?? '') || undefined,
        owner_email: String(f.get('owner') ?? ''),
        storage_gb: Number(f.get('limit') ?? 2),
      })
      onCreated(res.slug, res.name)
    } catch (err) {
      setError(errorMessage(err))
      setBusy(false)
    }
  }

  return (
    <Modal label={t('newCompany.title')} onClose={onClose}>
      <h2>{t('newCompany.title')}</h2>
      <p className="muted">{t('newCompany.lead')}</p>
      <form onSubmit={submit} noValidate>
        <div className="field">
          <label htmlFor="n-name">{t('newCompany.name')}</label>
          <input type="text" id="n-name" name="name" placeholder={t('newCompany.namePlaceholder')} required />
        </div>
        <div className="field">
          <label htmlFor="n-slug">{t('newCompany.slug')}</label>
          <input type="text" id="n-slug" name="slug" placeholder="acme-studio" className="mono" />
        </div>
        <div className="field">
          <label htmlFor="n-own">{t('newCompany.owner')}</label>
          <input type="email" id="n-own" name="owner" placeholder="owner@example.com" required />
        </div>
        <div className="field">
          <label htmlFor="n-lim">{t('newCompany.limit')}</label>
          <input type="number" id="n-lim" name="limit" defaultValue={2} min={1} style={{ maxWidth: 140 }} />
        </div>
        <ErrorNote message={error} />
        <div className="acts">
          <button type="button" className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button type="submit" className="btn primary" disabled={busy}>
            {t('newCompany.create')}
          </button>
        </div>
      </form>
    </Modal>
  )
}
