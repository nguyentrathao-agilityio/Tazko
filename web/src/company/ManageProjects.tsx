import { useState, type FormEvent } from 'react'
import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { Modal } from '@/ui/Modal'
import { displayName } from '@/ui/names'
import { useLoad } from '@/ui/useLoad'
import { NoAccess } from '@/company/CompanyHome'
import type { ManagedProject, Person } from '@/projects/types'
import { useRun } from '@/projects/useRun'
import { companyRoleLabel, isOwner } from '@/roles'
import { ErrorNote } from '@/ui/ErrorNote'
import { companyApi } from '@/paths'

type Dialog = { kind: 'new' } | { kind: 'owner'; project: ManagedProject } | { kind: 'delete'; project: ManagedProject } | null

export function ManageProjects({ company }: { company: Company }) {
  if (!isOwner(company.role)) return <NoAccess company={company} />
  return <ProjectList company={company} />
}

function ProjectList({ company }: { company: Company }) {
  const path = `${companyApi(company.slug)}/projects`
  const { data, error: loadError, reload } = useLoad(() => api<{ projects: ManagedProject[]; people: Person[] }>('GET', path), path)
  const [dialog, setDialog] = useState<Dialog>(null)
  const { error, runAndToast } = useRun(reload)

  const closeAfter = (fn: () => Promise<unknown>, message: string) =>
    runAndToast(fn, message).then((ok) => {
      if (ok) setDialog(null)
    })

  const archive = (p: ManagedProject) => {
    const to = p.status === 'archived' ? 'restore' : 'archive'
    void runAndToast(() => api('POST', `${path}/${p.id}/${to}`), t(to === 'archive' ? 'manage.archived' : 'manage.restored', { project: p.name }))
  }

  return (
    <>
      <div className="head">
        <div>
          <h1>{t('nav.manageProjects')}</h1>
          <p className="muted">{t('manage.lead', { company: company.name })}</p>
        </div>
        <span className="sp" />
        <button type="button" className="btn primary" onClick={() => setDialog({ kind: 'new' })} disabled={!data}>
          <Icon name="plus" />
          {t('companyHome.newProject')}
        </button>
      </div>
      <div className="note" style={{ margin: '0 0 16px' }}>
        {t('manage.note')}
      </div>
      <ErrorNote message={error ?? loadError} />
      <div className="tw">
        <table>
          <thead>
            <tr>
              <th>{t('projects.col.project')}</th>
              <th>{t('projects.col.owners')}</th>
              <th className="num">{t('companies.col.members')}</th>
              <th className="num">{t('companies.col.boards')}</th>
              <th>{t('manage.col.status')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {data && data.projects.length === 0 && (
              <tr>
                <td colSpan={6} className="muted">
                  {t('manage.empty')}
                </td>
              </tr>
            )}
            {data?.projects.map((p) => (
              <tr key={p.id}>
                <td>
                  <b>{p.name}</b>
                </td>
                <td>
                  {p.owners.length === 0 && <span className="pill warn">{t('manage.noOwner')}</span>}
                  {p.owners.map((o) => (
                    <span key={o.id} className="pill" style={{ margin: '2px 4px 2px 0' }}>
                      {displayName(o.name, o.email)}
                    </span>
                  ))}{' '}
                  <button type="button" className="btn sm quiet" onClick={() => setDialog({ kind: 'owner', project: p })} aria-label={t('manage.addOwnerTo', { project: p.name })}>
                    <Icon name="plus" />
                    {t('manage.owner')}
                  </button>
                </td>
                <td className="num">{p.members}</td>
                <td className="num">{p.boards}</td>
                <td>
                  {p.status === 'archived' ? <span className="pill warn">{t('manage.archivedPill')}</span> : <span className="pill ok">{t('status.active')}</span>}
                </td>
                <td>
                  <div className="row" style={{ flexWrap: 'nowrap' }}>
                    <button type="button" className="btn sm" onClick={() => archive(p)}>
                      {p.status === 'archived' ? t('manage.restore') : t('manage.archive')}
                    </button>
                    <button type="button" className="btn sm danger" onClick={() => setDialog({ kind: 'delete', project: p })}>
                      {t('manage.delete')}
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {dialog?.kind === 'new' && data && (
        <NewProject
          people={data.people}
          onClose={() => setDialog(null)}
          onCreate={(name, owner) => closeAfter(() => api('POST', path, { name, owner_id: owner }), t('manage.created', { project: name }))}
        />
      )}
      {dialog?.kind === 'owner' && data && (
        <AddOwner
          project={dialog.project}
          people={data.people.filter((m) => !dialog.project.owners.some((o) => o.id === m.id))}
          onClose={() => setDialog(null)}
          onAdd={(id) => closeAfter(() => api('POST', `${path}/${dialog.project.id}/owners`, { user_id: id }), t('manage.ownerAdded', { project: dialog.project.name }))}
        />
      )}
      {dialog?.kind === 'delete' && (
        <DeleteProject
          project={dialog.project}
          onClose={() => setDialog(null)}
          onDelete={(confirm) => closeAfter(() => api('DELETE', `${path}/${dialog.project.id}`, { confirm }), t('manage.deleted', { project: dialog.project.name }))}
        />
      )}
    </>
  )
}

function PersonOptions({ people }: { people: Person[] }) {
  return (
    <>
      {people.map((m) => (
        <option key={m.id} value={m.id}>
          {displayName(m.name, m.email)} ({m.name ? m.email : t(companyRoleLabel(m.role))})
        </option>
      ))}
    </>
  )
}

function NewProject({ people, onClose, onCreate }: { people: Person[]; onClose: () => void; onCreate: (name: string, owner: number) => void }) {
  const [problem, setProblem] = useState<string | null>(null)
  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const f = new FormData(e.currentTarget)
    const name = String(f.get('name') ?? '').trim()
    if (!name) {
      setProblem(t('manage.nameNeeded'))
      return
    }

    onCreate(name, Number(f.get('owner')))
  }

  return (
    <Modal label={t('companyHome.newProject')} onClose={onClose}>
      <h2>{t('companyHome.newProject')}</h2>
      <p className="muted">{t('manage.newLead')}</p>
      <form onSubmit={submit} noValidate>
        <div className="field">
          <label htmlFor="p-name">{t('project.name')}</label>
          <input type="text" id="p-name" name="name" placeholder={t('manage.namePlaceholder')} />
          {problem && (
            <p className="error" role="alert">
              {problem}
            </p>
          )}
        </div>
        <div className="field">
          <label htmlFor="p-own">{t('manage.firstOwner')}</label>
          <select id="p-own" name="owner" style={{ width: '100%' }}>
            <PersonOptions people={people} />
          </select>
        </div>
        <div className="acts">
          <button type="button" className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" type="submit">
            {t('manage.create')}
          </button>
        </div>
      </form>
    </Modal>
  )
}

function AddOwner({ project, people, onClose, onAdd }: { project: ManagedProject; people: Person[]; onClose: () => void; onAdd: (id: number) => void }) {
  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    onAdd(Number(new FormData(e.currentTarget).get('who')))
  }

  return (
    <Modal label={t('manage.addOwnerTo', { project: project.name })} onClose={onClose}>
      <h2>{t('manage.addOwnerTo', { project: project.name })}</h2>
      <p className="muted">{t('manage.addOwnerLead')}</p>
      <form onSubmit={submit}>
        <div className="field">
          <label htmlFor="a-who">{t('manage.person')}</label>
          <select id="a-who" name="who" style={{ width: '100%' }}>
            <PersonOptions people={people} />
          </select>
        </div>
        <div className="acts">
          <button type="button" className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" type="submit" disabled={people.length === 0}>
            {t('manage.addAsOwner')}
          </button>
        </div>
      </form>
    </Modal>
  )
}

function DeleteProject({ project, onClose, onDelete }: { project: ManagedProject; onClose: () => void; onDelete: (confirm: string) => void }) {
  const [typed, setTyped] = useState('')
  return (
    <Modal label={t('manage.deleteTitle', { project: project.name })} onClose={onClose}>
      <h2>{t('manage.deleteTitle', { project: project.name })}</h2>
      <p className="muted">{t('manage.deleteLead')}</p>
      <div className="field">
        <label htmlFor="p-t">{t('manage.typeToConfirm', { project: project.name })}</label>
        <input type="text" id="p-t" autoComplete="off" value={typed} onChange={(e) => setTyped(e.target.value)} />
      </div>
      <div className="acts">
        <button type="button" className="btn" onClick={onClose}>
          {t('common.cancel')}
        </button>
        <button type="button" className="btn danger" disabled={typed.trim() !== project.name} onClick={() => onDelete(typed.trim())}>
          {t('manage.deleteButton')}
        </button>
      </div>
    </Modal>
  )
}
