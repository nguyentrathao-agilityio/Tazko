import { useState, type FormEvent } from 'react'
import { api } from '@/http'
import { t, type MessageKey } from '@/i18n'
import { shortDate } from '@/card/look'
import { useRun } from '@/projects/useRun'
import { ErrorNote } from '@/ui/ErrorNote'
import { when } from '@/ui/format'
import { Icon } from '@/ui/Icon'
import { Modal } from '@/ui/Modal'
import { useLoad } from '@/ui/useLoad'
import { SecretModal } from '@/tokens/SecretModal'
import { EXPIRY, SCOPES, type Scope, type TokenRow } from '@/tokens/types'

const scopeText = (s: Scope) => t(`tokens.scope.${s}` as MessageKey)

const expires = (unix: number | null) => (unix ? shortDate(unix) : t('tokens.never'))

type Project = { id: number; name: string }

export function TokenTable({
  path,
  title,
  lead,
  scopes = SCOPES,
  projects,
  footer,
}: {
  path: string
  title: string
  lead: string
  scopes?: Scope[]
  projects?: Project[]
  footer?: React.ReactNode
}) {
  const { data, error: loadError, reload } = useLoad(() => api<{ tokens: TokenRow[] }>('GET', path), path)
  const { error, runAndToast } = useRun(reload)
  const creating = useRun(reload)
  const [busy, setBusy] = useState(false)
  const [making, setMaking] = useState(false)
  const [made, setMade] = useState<{ name: string; token: string } | null>(null)
  const company = projects !== undefined
  const close = () => {
    setMaking(false)
    creating.setError(null)
  }

  const create = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const f = new FormData(e.currentTarget)
    const name = String(f.get('name') ?? '').trim()
    if (!name) {
      e.currentTarget.querySelector<HTMLInputElement>('#tk-n')?.focus()
      return
    }

    if (busy) return
    const days = Number(f.get('expires'))
    const pick = String(f.get('projects') ?? '')
    const body: Record<string, unknown> = { name, scope: f.get('scope'), expires_in_days: days || undefined }
    if (company) body.projects = pick ? [Number(pick)] : (projects ?? []).map((p) => p.id)
    setBusy(true)
    void creating
      .run(async () => {
        const r = await api<{ token: string }>('POST', path, body)
        setMaking(false)
        setMade({ name, token: r.token })
      })
      .finally(() => setBusy(false))
  }

  return (
    <section className="box">
      <div className="row">
        <div className="grow">
          <h2 style={{ margin: 0 }}>{title}</h2>
          <p className="muted" style={{ margin: '4px 0 0' }}>
            {lead}
          </p>
        </div>
        <button type="button" className="btn primary" onClick={() => setMaking(true)}>
          <Icon name="plus" />
          {t('tokens.new')}
        </button>
      </div>
      <ErrorNote message={error ?? loadError} />
      <div className="tw" style={{ marginTop: 14 }}>
        <table>
          <thead>
            <tr>
              <th>{t('tokens.name')}</th>
              <th>{t('tokens.access')}</th>
              {company ? <th>{t('tokens.projects')}</th> : <th>{t('tokens.created')}</th>}
              <th>{t('tokens.lastUsed')}</th>
              <th>{t('tokens.expires')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {data?.tokens.map((row) => (
              <tr key={row.id}>
                <td>
                  <b>{row.name}</b>
                  {company && <small className="muted" style={{ display: 'block' }}>{t('tokens.createdBy', { date: shortDate(row.created_at), by: row.by ?? '' })}</small>}
                </td>
                <td>{scopeText(row.scope)}</td>
                {company ? <td>{(row.project_names ?? []).join(', ')}</td> : <td className="muted">{shortDate(row.created_at)}</td>}
                <td className="muted">{row.last_used_at ? when(row.last_used_at) : t('tokens.notUsed')}</td>
                <td className="muted">{expires(row.expires_at)}</td>
                <td className="num">
                  <button type="button" className="btn sm danger" onClick={() => void runAndToast(() => api('DELETE', `${path}/${row.id}`), t('tokens.revoked', { name: row.name }))}>
                    {t('tokens.revoke')}
                  </button>
                </td>
              </tr>
            ))}
            {data?.tokens.length === 0 && (
              <tr>
                <td colSpan={6} className="muted">
                  {t('tokens.none')}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      {footer}
      {making && (
        <Modal label={t(company ? 'tokens.newCompany' : 'tokens.newPersonal')} onClose={close}>
          <h2>{t(company ? 'tokens.newCompany' : 'tokens.newPersonal')}</h2>
          <form onSubmit={create} noValidate>
            <div className="field">
              <label htmlFor="tk-n">{t('tokens.name')}</label>
              <input type="text" id="tk-n" name="name" placeholder={t('tokens.namePlaceholder')} maxLength={60} required />
            </div>
            <div className="field">
              <label htmlFor="tk-s">{t('tokens.access')}</label>
              <select id="tk-s" name="scope" style={{ width: '100%' }} defaultValue="read">
                {scopes.map((s) => (
                  <option key={s} value={s}>
                    {scopeText(s)}
                  </option>
                ))}
              </select>
            </div>
            {company && (
              <div className="field">
                <label htmlFor="tk-p">{t('tokens.projects')}</label>
                <select id="tk-p" name="projects" style={{ width: '100%' }}>
                  {Boolean(projects?.length) && <option value="">{t('tokens.allMyProjects')}</option>}
                  {projects?.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
                <small className="hint">{t('tokens.projectsHint')}</small>
              </div>
            )}
            <div className="field">
              <label htmlFor="tk-x">{t('tokens.expires')}</label>
              <select id="tk-x" name="expires" style={{ width: '100%' }} defaultValue="90">
                {EXPIRY.map((d) => (
                  <option key={d} value={d}>
                    {d ? t('tokens.inDays', { n: d }) : t('tokens.never')}
                  </option>
                ))}
              </select>
            </div>
            <ErrorNote message={creating.error} />
            <div className="acts">
              <button type="button" className="btn" onClick={close}>
                {t('common.cancel')}
              </button>
              <button type="submit" className="btn primary" disabled={busy}>
                {t('tokens.create')}
              </button>
            </div>
          </form>
        </Modal>
      )}
      {made && <SecretModal title={t('tokens.made')} lead={t('tokens.madeLead', { name: made.name })} label={t('tokens.secret')} text={made.token} onClose={() => setMade(null)} />}
    </section>
  )
}
