import { useState } from 'react'
import { api } from '@/http'
import { t, type MessageKey } from '@/i18n'
import { when } from '@/ui/format'
import { useLoad } from '@/ui/useLoad'
import type { AuditEntry } from '@/admin/types'
import { ErrorNote } from '@/ui/ErrorNote'
import { ADMIN_API } from '@/paths'

const PAGE = 100

const filters: [string, MessageKey][] = [
  ['all', 'audit.all'],
  ['refused', 'audit.refused'],
  ['company', 'audit.company'],
  ['projects', 'audit.projects'],
  ['invites', 'audit.invites'],
  ['people', 'audit.people'],
  ['integrations', 'audit.integrations'],
]

function describe(e: AuditEntry): string {
  const key = `action.${e.action}` as MessageKey
  const text = t(key)
  return text === key ? e.action.replace(/_/g, ' ') : text
}

function page(kind: string, before?: number) {
  const q = new URLSearchParams({ kind })
  if (before) q.set('before', String(before))
  return api<{ entries: AuditEntry[] }>('GET', `${ADMIN_API}/audit?${q}`).then((r) => r.entries)
}

export function Audit() {
  const [kind, setKind] = useState('all')

  return (
    <>
      <div className="head">
        <div>
          <h1>{t('rail.audit')}</h1>
          <p className="muted">{t('audit.lead')}</p>
        </div>
        <span className="sp" />
        <select aria-label={t('audit.filter')} value={kind} onChange={(e) => setKind(e.target.value)}>
          {filters.map(([v, label]) => (
            <option key={v} value={v}>
              {t(label)}
            </option>
          ))}
        </select>
      </div>
      <AuditList key={kind} kind={kind} />
    </>
  )
}

function AuditList({ kind }: { kind: string }) {
  const { data: first, error } = useLoad(() => page(kind), kind)
  const [older, setOlder] = useState<AuditEntry[]>([])
  const [ended, setEnded] = useState(false)
  const entries = [...(first ?? []), ...older]
  const more = !ended && entries.length > 0 && entries.length % PAGE === 0

  const loadMore = async () => {
    const next = await page(kind, entries[entries.length - 1]?.id)
    setOlder((o) => [...o, ...next])
    if (next.length < PAGE) setEnded(true)
  }

  return (
    <>
      <ErrorNote message={error} />
      <div className="tw">
        <ul className="log">
          {first && entries.length === 0 && <li className="muted">{t('audit.none')}</li>}
          {entries.map((e) => (
            <li key={e.id}>
              <time dateTime={new Date(e.at * 1000).toISOString()}>{when(e.at)}</time>
              <span>
                {e.kind === 'refused' ? (
                  <>
                    <b>{e.target}</b> {describe(e)} <span className="pill bad">{t('audit.refusedPill')}</span>
                  </>
                ) : (
                  <>
                    <b>{e.actor ?? t('audit.system')}</b> {describe(e)} {e.target && <b>{e.target}</b>}
                    {e.company && e.kind !== 'company' && <span className="muted"> · {e.company}</span>}
                  </>
                )}
              </span>
            </li>
          ))}
        </ul>
      </div>
      {more && (
        <button type="button" className="btn" onClick={() => void loadMore()}>
          {t('common.loadMore')}
        </button>
      )}
    </>
  )
}
