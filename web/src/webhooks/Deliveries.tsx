import { api } from '@/http'
import { t } from '@/i18n'
import { useRun } from '@/projects/useRun'
import { ErrorNote } from '@/ui/ErrorNote'
import { when } from '@/ui/format'
import { useLoad } from '@/ui/useLoad'
import type { Delivery, Webhook } from '@/webhooks/types'

export function Deliveries({ path, hook, onRemoved }: { path: string; hook: Webhook; onRemoved: () => void }) {
  const base = `${path}/${hook.id}`
  const { data, error: loadError, reload } = useLoad(() => api<{ deliveries: Delivery[] }>('GET', `${base}/deliveries`), `${base}/deliveries`)
  const { error, runAndToast } = useRun(reload)
  const removing = useRun(onRemoved)

  return (
    <div className="hookd">
      <div className="row" style={{ marginBottom: 8 }}>
        <small className="muted" style={{ flex: 1 }}>
          {hook.events.join(' · ')}
        </small>
        <button type="button" className="btn sm" disabled={!hook.active} title={hook.active ? undefined : t('webhooks.resumeFirst')} onClick={() => void runAndToast(() => api('POST', `${base}/test`), t('webhooks.testQueued'))}>
          {t('webhooks.test')}
        </button>
        <button type="button" className="btn sm danger" onClick={() => void removing.runAndToast(() => api('DELETE', base), t('webhooks.deleted'))}>
          {t('webhooks.delete')}
        </button>
      </div>
      <ErrorNote message={error ?? removing.error ?? loadError} />
      <div className="tw">
        <table>
          <thead>
            <tr>
              <th>{t('webhooks.when')}</th>
              <th>{t('webhooks.event')}</th>
              <th>{t('webhooks.result')}</th>
              <th className="num">{t('webhooks.time')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {data?.deliveries.map((d) => (
              <tr key={d.id}>
                <td className="muted">{when(d.finished_at ?? d.created_at)}</td>
                <td className="mono">{d.event}</td>
                <td>
                  <Result d={d} />
                </td>
                <td className="num muted">{d.ms === null ? '' : t('webhooks.ms', { n: d.ms })}</td>
                <td className="num">
                  {d.status === 'failed' && hook.active && (
                    <button type="button" className="btn sm" onClick={() => void runAndToast(() => api('POST', `${base}/deliveries/${d.id}/redeliver`), t('webhooks.redelivering'))}>
                      {t('webhooks.redeliver')}
                    </button>
                  )}
                </td>
              </tr>
            ))}
            {data?.deliveries.length === 0 && (
              <tr>
                <td colSpan={5} className="muted">
                  {t('webhooks.noDeliveries')}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <p className="hint" style={{ marginTop: 8 }}>
        {t('webhooks.signatureHint')}
      </p>
    </div>
  )
}

function Result({ d }: { d: Delivery }) {
  if (d.attempts === 0) return <span className="pill">{t('webhooks.queued')}</span>

  const label = d.code === null ? t('webhooks.noAnswer') : String(d.code)
  return (
    <>
      <span className={`pill ${d.status === 'delivered' ? 'ok' : 'bad'}`} title={d.error ?? undefined}>
        {label}
      </span>
      {d.status === 'pending' && <small className="muted"> {t('webhooks.retrying', { when: when(d.next_attempt_at) })}</small>}
    </>
  )
}
