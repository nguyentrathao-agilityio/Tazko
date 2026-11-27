import { EmailInviteOption } from '@/invite/EmailInviteOption'
import { invitedMessage, useEmailEnabled, wantsEmail, type InviteResult } from '@/invite/invite'
import type { FormEvent } from 'react'
import { api } from '@/http'
import { t } from '@/i18n'
import { expiresIn, when } from '@/ui/format'
import { Icon } from '@/ui/Icon'
import { ErrorNote } from '@/ui/ErrorNote'
import { AccountPill, Person } from '@/ui/people'
import { useToast } from '@/ui/toast'
import { displayName } from '@/ui/names'
import type { Invite, Member } from '@/admin/types'
import { useRun } from '@/projects/useRun'
import { companyRoleLabel, MEMBER, OWNER } from '@/roles'

const roleChoices = [MEMBER, OWNER] as const

function RoleOptions() {
  return (
    <>
      {roleChoices.map((r) => (
        <option key={r} value={r}>
          {t(companyRoleLabel(r))}
        </option>
      ))}
    </>
  )
}

export function MembersPanel({
  base,
  revokePath,
  members,
  invites,
  onChange,
}: {
  base: string
  revokePath: (id: number) => string
  members: Member[]
  invites: Invite[]
  onChange: () => void
}) {
  const { error, run } = useRun(onChange)
  const toast = useToast()

  const reinvite = (email: string, role: string) => run(() => api('POST', `${base}/invites`, { email, role }))

  const setRole = async (m: Member, role: string) => {
    const name = displayName(m.name, m.email)
    if (!(await run(() => api('PATCH', `${base}/members/${m.id}`, { role })))) return
    toast(t('toast.roleChanged', { name, role: t(companyRoleLabel(role)).toLowerCase() }), () =>
      run(() => api('PATCH', `${base}/members/${m.id}`, { role: m.role })),
    )
  }

  const remove = async (m: Member) => {
    if (await run(() => api('DELETE', `${base}/members/${m.id}`))) toast(t('toast.removed', { name: displayName(m.name, m.email) }))
  }

  const revoke = async (v: Invite) => {
    if (!(await run(() => api('DELETE', revokePath(v.id))))) return
    toast(t('toast.revoked', { email: v.email }), () => reinvite(v.email, v.role))
  }

  const inviteHint = t(useEmailEnabled() ? 'members.inviteHintEmail' : 'members.inviteHint')

  const onInvite = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const form = e.currentTarget
    const f = new FormData(form)
    const email = String(f.get('email') ?? '').trim()
    void run(async () => {
      const res = await api<InviteResult>('POST', `${base}/invites`, { email, role: String(f.get('role') ?? MEMBER), send_email: wantsEmail(f) })
      form.reset()
      toast(invitedMessage(res, email, 'toast.added'))
    })
  }

  return (
    <>
      <ErrorNote message={error} />
      <section className="box">
        <h2>{t('members.invite')}</h2>
        <form className="row" onSubmit={onInvite} noValidate>
          <div className="grow">
            <input type="email" name="email" placeholder="name@example.com" aria-label={t('members.email')} required />
          </div>
          <select name="role" aria-label={t('members.role')} defaultValue={MEMBER}>
            <RoleOptions />
          </select>
          <EmailInviteOption />
          <button className="btn primary" type="submit">
            <Icon name="plus" />
            {t('members.inviteButton')}
          </button>
        </form>
        <p className="hint" style={{ marginTop: 10 }}>
          {inviteHint}
        </p>
      </section>
      <section className="box">
        <h2>
          {t('members.title')} <span className="muted">({members.length})</span>
        </h2>
        <div className="tw">
          <table>
            <thead>
              <tr>
                <th>{t('members.col.person')}</th>
                <th>{t('members.col.account')}</th>
                <th>{t('members.col.role')}</th>
                <th>{t('members.col.last')}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {members.length === 0 && (
                <tr>
                  <td colSpan={5} className="muted">
                    {t('members.none')}
                  </td>
                </tr>
              )}
              {members.map((m) => (
                <tr key={m.id}>
                  <td>
                    <Person name={m.name} email={m.email} />
                  </td>
                  <td>
                    <AccountPill email={m.email} />
                  </td>
                  <td>
                    <select
                      aria-label={`${t('members.col.role')}: ${displayName(m.name, m.email)}`}
                      value={m.role}
                      onChange={(e) => void setRole(m, e.target.value)}
                    >
                      <RoleOptions />
                    </select>
                  </td>
                  <td className="muted">{when(m.last_seen)}</td>
                  <td>
                    <button
                      type="button"
                      className="btn sm quiet"
                      aria-label={t('members.removeLabel', { name: displayName(m.name, m.email) })}
                      onClick={() => void remove(m)}
                    >
                      <Icon name="x" />
                      {t('common.remove')}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
      <section className="box">
        <h2>
          {t('invites.pending')} <span className="muted">({invites.length})</span>
        </h2>
        {invites.length === 0 ? (
          <p className="muted">{t('invites.none')}</p>
        ) : (
          <InviteTable invites={invites} withCompany={false} onRevoke={(v) => void revoke(v)} />
        )}
      </section>
    </>
  )
}

export function InviteTable({
  invites,
  withCompany,
  onRevoke,
}: {
  invites: Invite[]
  withCompany: boolean
  onRevoke: (invite: Invite) => void
}) {
  return (
    <div className="tw">
      <table>
        <thead>
          <tr>
            <th>{t('invites.col.email')}</th>
            <th>{t('members.col.account')}</th>
            {withCompany && <th>{t('invites.col.company')}</th>}
            <th>{t('members.col.role')}</th>
            <th>{t('invites.col.by')}</th>
            <th>{t('invites.col.date')}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {invites.map((v) => (
            <tr key={v.id}>
              <td>{v.email}</td>
              <td>
                <AccountPill email={v.email} />
              </td>
              {withCompany && <td>{v.company_name}</td>}
              <td>
                <span className="pill">{v.role}</span>
              </td>
              <td className="muted">{v.invited_by ?? '—'}</td>
              <td className="muted">
                {when(v.created_at)} · {expiresIn(v.expires_at)}
              </td>
              <td>
                <button type="button" className="btn sm" onClick={() => onRevoke(v)}>
                  {t('common.revoke')}
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
