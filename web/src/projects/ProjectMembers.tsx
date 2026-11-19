import type { FormEvent } from 'react'
import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { when } from '@/ui/format'
import { Icon } from '@/ui/Icon'
import { displayName } from '@/ui/names'
import { AccountPill, Avatar } from '@/ui/people'
import { useToast } from '@/ui/toast'
import { useLoad } from '@/ui/useLoad'
import type { Can, ProjectInvite, ProjectMember, ProjectRole, ProjectView } from '@/projects/types'
import { useRun } from '@/projects/useRun'
import { PROJECT_ROLES } from '@/roles'

export function ProjectMembers({ company, project, onChange }: { company: Company; project: ProjectView; onChange: () => void }) {
  const base = `/api/v1/c/${company.slug}/projects/${project.id}`
  const { data, reload } = useLoad(
    () => api<{ members: ProjectMember[]; invites: ProjectInvite[]; can: Can }>('GET', `${base}/members`),
    `${base}/members`,
  )
  const toast = useToast()
  const { error, run } = useRun(() => {
    reload()
    onChange()
  })
  const can = data?.can ?? project.can

  const onInvite = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const form = e.currentTarget
    const f = new FormData(form)
    const email = String(f.get('email') ?? '').trim()
    void run(async () => {
      const res = await api<{ result: string }>('POST', `${base}/invites`, { email, role: String(f.get('role') ?? 'member') })
      form.reset()
      toast(t(res?.result === 'added' ? 'project.toastAdded' : 'toast.invited', { email: email.toLowerCase() }))
    })
  }

  const setRole = (m: ProjectMember, role: string) =>
    void run(() => api('PATCH', `${base}/members/${m.id}`, { role })).then((ok) => {
      if (ok) toast(t('toast.roleChanged', { name: displayName(m.name, m.email), role: t(`projectRole.${role as ProjectRole}`).toLowerCase() }))
    })

  const remove = (m: ProjectMember) =>
    void run(() => api('DELETE', `${base}/members/${m.id}`)).then((ok) => {
      if (ok) toast(t('toast.removed', { name: displayName(m.name, m.email) }))
    })

  const revoke = (v: ProjectInvite) =>
    void run(() => api('DELETE', `${base}/invites/${v.id}`)).then((ok) => {
      if (ok) toast(t('toast.revoked', { email: v.email }))
    })

  return (
    <>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {can.invite ? (
        <section className="box">
          <h2>{t('project.invite')}</h2>
          <form className="row" onSubmit={onInvite} noValidate>
            <div className="grow">
              <input type="email" name="email" placeholder="name@example.com" aria-label={t('members.email')} />
            </div>
            <select name="role" aria-label={t('members.role')} defaultValue="member">
              <option value="member">{t('projectRole.member')}</option>
              <option value="observer">{t('projectRole.observer')}</option>
              {can.manage && <option value="owner">{t('projectRole.owner')}</option>}
            </select>
            <button className="btn primary" type="submit">
              <Icon name="plus" />
              {t('project.inviteButton')}
            </button>
          </form>
          <p className="hint" style={{ marginTop: 10 }}>
            {t('project.inviteHint', { company: company.name })}
          </p>
        </section>
      ) : (
        <div className="note" style={{ margin: '0 0 16px' }}>
          {project.members_invite ? t('project.inviteOwnersOnly') : t('project.inviteOwnersOnlySwitch')}
        </div>
      )}
      <section className="box">
        <h2>
          {t('project.members')} <span className="muted">({data?.members.length ?? project.member_count})</span>
        </h2>
        <div className="tw">
          <table>
            <thead>
              <tr>
                <th>{t('members.col.person')}</th>
                <th>{t('members.col.account')}</th>
                <th>{t('members.col.role')}</th>
                <th>{t('project.col.lastActive')}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {data?.members.map((m) => {
                const name = displayName(m.name, m.email)
                return (
                  <tr key={m.id}>
                    <td>
                      <div className="who">
                        <Avatar name={name} />
                        <div>
                          <b>
                            {name}
                            {m.you && ` ${t('project.you')}`}
                          </b>
                          {name !== m.email && <small>{m.email}</small>}
                        </div>
                      </div>
                    </td>
                    <td>
                      <AccountPill email={m.email} />
                    </td>
                    <td>
                      {can.manage ? (
                        <select aria-label={t('project.roleOf', { name })} value={m.role} onChange={(e) => setRole(m, e.target.value)}>
                          {PROJECT_ROLES.map((r) => (
                            <option key={r} value={r}>
                              {t(`projectRole.${r}`)}
                            </option>
                          ))}
                        </select>
                      ) : (
                        <span className="pill">{t(`projectRole.${m.role as ProjectRole}`)}</span>
                      )}
                    </td>
                    <td className="muted">{m.last_active ? when(m.last_active) : t('common.never')}</td>
                    <td>
                      {can.manage && (
                        <button type="button" className="btn sm quiet" onClick={() => remove(m)} aria-label={t('members.removeLabel', { name })}>
                          <Icon name="x" />
                          {t('common.remove')}
                        </button>
                      )}
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      </section>
      <section className="box">
        <h2>
          {t('project.pending')} <span className="muted">({data?.invites.length ?? 0})</span>
        </h2>
        {data && data.invites.length === 0 && <p className="muted">{t('project.noInvites')}</p>}
        {data && data.invites.length > 0 && (
          <div className="tw">
            <table>
              <thead>
                <tr>
                  <th>{t('members.email')}</th>
                  <th>{t('members.col.role')}</th>
                  <th>{t('project.col.invitedBy')}</th>
                  <th>{t('project.col.date')}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {data.invites.map((v) => (
                  <tr key={v.id}>
                    <td>{v.email}</td>
                    <td>
                      <span className="pill">{t(`projectRole.${v.role}`)}</span>
                    </td>
                    <td className="muted">{v.invited_by ?? '—'}</td>
                    <td className="muted">{when(v.created_at)}</td>
                    <td>
                      {can.invite && (
                        <button type="button" className="btn sm" onClick={() => revoke(v)}>
                          {t('common.revoke')}
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </>
  )
}
