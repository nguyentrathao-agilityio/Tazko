import { useState, type FormEvent } from 'react'
import { api, HttpError } from '@/http'
import { t } from '@/i18n'
import { Link, type Go } from '@/ui/Link'
import { Modal } from '@/ui/Modal'
import { Avatar } from '@/ui/people'
import { useToast } from '@/ui/toast'
import { useLoad } from '@/ui/useLoad'
import { StatusPill } from '@/admin/Companies'
import { MembersPanel } from '@/admin/MembersPanel'
import type { CompanyDetail as Detail, ProjectRow } from '@/admin/types'

const GB = 1024 ** 3
type Tab = 'members' | 'projects' | 'settings'

export function CompanyDetail({ slug, go }: { slug: string; go: Go }) {
  const base = `/api/v1/admin/companies/${encodeURIComponent(slug)}`
  const { data: c, error, reload } = useLoad(() => api<Detail>('GET', base), base)
  const [tab, setTab] = useState<Tab>('members')
  const [problem, setProblem] = useState<string | null>(null)
  const toast = useToast()

  const status = async (to: 'suspend' | 'resume', undoable = true) => {
    setProblem(null)
    try {
      await api('POST', `${base}/${to}`)
      reload()
      const name = c?.name ?? slug
      const back = to === 'suspend' ? 'resume' : 'suspend'
      toast(t(to === 'suspend' ? 'toast.suspended' : 'toast.resumed', { company: name }), undoable ? () => status(back, false) : undefined)
    } catch (e) {
      setProblem(e instanceof HttpError ? e.message : String(e))
    }
  }

  if (error) return <p className="error" role="alert">{error}</p>
  if (!c) return <p className="muted">{t('common.loading')}</p>

  return (
    <>
      <div className="crumb">
        <Link to="/admin/companies" go={go}>
          {t('rail.companies')}
        </Link>{' '}
        / {c.name}
      </div>
      <div className="head">
        <div className="who">
          <Avatar name={c.name} square big />
          <div>
            <h1>{c.name}</h1>
            <small className="mono muted">{c.slug}</small>
          </div>
        </div>
        <StatusPill status={c.status} />
        <span className="sp" />
        {c.status === 'active' ? (
          <button type="button" className="btn" onClick={() => void status('suspend')}>
            {t('company.suspend')}
          </button>
        ) : (
          <button type="button" className="btn primary" onClick={() => void status('resume')}>
            {t('company.resume')}
          </button>
        )}
      </div>
      {problem && <p className="error" role="alert">{problem}</p>}
      <div className="tabs" role="tablist">
        {(['members', 'projects', 'settings'] as Tab[]).map((k) => (
          <button
            key={k}
            type="button"
            role="tab"
            aria-selected={tab === k}
            className={`tab${tab === k ? ' on' : ''}`}
            onClick={() => setTab(k)}
          >
            {t(`company.tab.${k}`)}
          </button>
        ))}
      </div>
      {tab === 'members' && (
        <>
          <div className="note" style={{ margin: '0 0 16px' }}>
            {t('company.adminNote')}
          </div>
          <MembersPanel
            base={base}
            revokePath={(id) => `/api/v1/admin/invites/${id}`}
            members={c.members}
            invites={c.invites}
            onChange={reload}
          />
        </>
      )}
      {tab === 'projects' && <ProjectsPanel projects={c.projects} />}
      {tab === 'settings' && <SettingsPanel company={c} base={base} onSaved={reload} onDeleted={() => {
            toast(t('toast.companyDeleted', { company: c.name }))
            go('/admin/companies')
          }} />}
    </>
  )
}

function ProjectsPanel({ projects }: { projects: ProjectRow[] }) {
  return (
    <>
      <div className="note" style={{ margin: '0 0 16px' }}>
        {t('projects.note')}
      </div>
      <div className="tw">
        <table>
          <thead>
            <tr>
              <th>{t('projects.col.project')}</th>
              <th className="num">{t('companies.col.members')}</th>
              <th className="num">{t('companies.col.boards')}</th>
              <th>{t('projects.col.owners')}</th>
            </tr>
          </thead>
          <tbody>
            {projects.length === 0 ? (
              <tr>
                <td colSpan={4} className="muted">
                  {t('projects.none')}
                </td>
              </tr>
            ) : (
              projects.map((p) => (
                <tr key={p.name}>
                  <td>
                    <b>{p.name}</b>
                  </td>
                  <td className="num">{p.members}</td>
                  <td className="num">{p.boards}</td>
                  <td>{p.owners.length ? p.owners.join(', ') : <span className="pill warn">{t('projects.noOwner')}</span>}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </>
  )
}

function SettingsPanel({
  company,
  base,
  onSaved,
  onDeleted,
}: {
  company: Detail
  base: string
  onSaved: () => void
  onDeleted: () => void
}) {
  const [msg, setMsg] = useState<string | null>(null)
  const [deleting, setDeleting] = useState(false)
  const toast = useToast()

  const save = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const f = new FormData(e.currentTarget)
    try {
      await api('PATCH', base, { name: String(f.get('name') ?? ''), storage_gb: Number(f.get('limit')) })
      setMsg(null)
      toast(t('toast.saved'))
      onSaved()
    } catch (err) {
      setMsg(err instanceof HttpError ? err.message : String(err))
    }
  }

  return (
    <>
      <section className="box">
        <h2>{t('company.general')}</h2>
        <form onSubmit={save} noValidate>
          <div className="field">
            <label htmlFor="s-name">{t('company.name')}</label>
            <input type="text" id="s-name" name="name" defaultValue={company.name} />
          </div>
          <div className="field">
            <label htmlFor="s-lim">{t('company.limit')}</label>
            <input
              type="number"
              id="s-lim"
              name="limit"
              min={1}
              defaultValue={Math.round((company.storage_limit ?? 2 * GB) / GB)}
              style={{ maxWidth: 140 }}
            />
          </div>
          {msg && (
            <p className="error" role="alert" style={{ margin: '0 0 12px' }}>
              {msg}
            </p>
          )}
          <button className="btn primary" type="submit">
            {t('common.save')}
          </button>
        </form>
      </section>
      <section className="box danger-zone">
        <h2>{t('company.delete.title')}</h2>
        <p className="muted">{t('company.delete.lead')}</p>
        <button type="button" className="btn danger" onClick={() => setDeleting(true)}>
          {t('company.delete.button')}
        </button>
      </section>
      {deleting && <DeleteCompany company={company} base={base} onClose={() => setDeleting(false)} onDeleted={onDeleted} />}
    </>
  )
}

function DeleteCompany({
  company,
  base,
  onClose,
  onDeleted,
}: {
  company: Detail
  base: string
  onClose: () => void
  onDeleted: () => void
}) {
  const [typed, setTyped] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [before, after] = t('deleteCompany.confirm').split('{slug}')

  const go = async () => {
    try {
      await api('DELETE', base, { confirm: typed })
      onDeleted()
    } catch (e) {
      setError(e instanceof HttpError ? e.message : String(e))
    }
  }

  return (
    <Modal label={t('deleteCompany.title', { name: company.name })} onClose={onClose}>
      <h2>{t('deleteCompany.title', { name: company.name })}</h2>
      <p className="muted">{t('deleteCompany.lead')}</p>
      <div className="field">
        <label htmlFor="d-t">
          {before}
          <b className="mono">{company.slug}</b>
          {after}
        </label>
        <input type="text" id="d-t" autoComplete="off" value={typed} onChange={(e) => setTyped(e.target.value)} />
      </div>
      {error && <p className="error" role="alert">{error}</p>}
      <div className="acts">
        <button type="button" className="btn" onClick={onClose}>
          {t('common.cancel')}
        </button>
        <button type="button" className="btn danger" disabled={typed.trim() !== company.slug} onClick={() => void go()}>
          {t('deleteCompany.go')}
        </button>
      </div>
    </Modal>
  )
}
