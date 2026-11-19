import { useState } from 'react'
import { api } from '@/http'
import { t } from '@/i18n'
import { when } from '@/ui/format'
import { ErrorNote } from '@/ui/ErrorNote'
import { AccountPill, Person } from '@/ui/people'
import { displayName } from '@/ui/names'
import { PageHead } from '@/ui/PageHead'
import { useToast } from '@/ui/toast'
import { useLoad } from '@/ui/useLoad'
import type { PersonRow } from '@/admin/types'
import { useRun } from '@/projects/useRun'
import { ADMIN_API } from '@/paths'

export function People() {
  const { data, error, reload } = useLoad(() => api<{ people: PersonRow[] }>('GET', `${ADMIN_API}/people`), 'people')
  const [q, setQ] = useState('')
  const { error: problem, run } = useRun(reload)
  const toast = useToast()
  const needle = q.trim().toLowerCase()
  const list = (data?.people ?? []).filter(
    (p) => !needle || p.name.toLowerCase().includes(needle) || p.email.toLowerCase().includes(needle),
  )

  const change = async (p: PersonRow, body: Partial<Pick<PersonRow, 'is_super_admin' | 'disabled'>>, undoable = true) => {
    if (!(await run(() => api('PATCH', `${ADMIN_API}/people/${p.id}`, body)))) return

    const name = displayName(p.name, p.email)
    const message =
      body.is_super_admin !== undefined
        ? t(body.is_super_admin ? 'toast.superOn' : 'toast.superOff', { name })
        : t(body.disabled ? 'toast.activeOff' : 'toast.activeOn', { name })
    const back = body.is_super_admin !== undefined ? { is_super_admin: p.is_super_admin } : { disabled: p.disabled }
    toast(message, undoable ? () => change(p, back, false) : undefined)
  }

  return (
    <>
      <PageHead title={t('rail.people')} lead={t('people.lead')}>
        <div style={{ width: 260, maxWidth: '100%' }}>
          <input type="search" placeholder={t('people.search')} aria-label={t('people.search')} value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
      </PageHead>
      <ErrorNote message={error ?? problem} />
      <div className="tw">
        <table>
          <thead>
            <tr>
              <th>{t('members.col.person')}</th>
              <th>{t('members.col.account')}</th>
              <th>{t('people.col.companies')}</th>
              <th>{t('members.col.last')}</th>
              <th>{t('people.col.super')}</th>
              <th>{t('people.col.active')}</th>
            </tr>
          </thead>
          <tbody>
            {list.map((p) => {
              const name = displayName(p.name, p.email)
              return (
                <tr key={p.id}>
                  <td>
                    <Person name={p.name} email={p.email} />
                  </td>
                  <td>
                    <AccountPill email={p.email} />
                  </td>
                  <td>
                    {p.companies.map((c) => (
                      <span key={c} className="pill" style={{ margin: '2px 4px 2px 0' }}>
                        {c}
                      </span>
                    ))}
                  </td>
                  <td className="muted">{when(p.last_seen)}</td>
                  <td>
                    <button
                      type="button"
                      className="switchbtn"
                      role="switch"
                      aria-checked={p.is_super_admin}
                      aria-label={t('people.superFor', { name })}
                      onClick={() => void change(p, { is_super_admin: !p.is_super_admin })}
                    />
                  </td>
                  <td>
                    <button
                      type="button"
                      className="switchbtn"
                      role="switch"
                      aria-checked={!p.disabled}
                      aria-label={t('people.activeFor', { name })}
                      onClick={() => void change(p, { disabled: !p.disabled })}
                    />
                  </td>
                </tr>
              )
            })}
            {data && list.length === 0 && (
              <tr>
                <td colSpan={6} className="muted">
                  {t('people.none')}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </>
  )
}
