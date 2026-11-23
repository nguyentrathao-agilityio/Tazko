import { api } from '@/http'
import { t } from '@/i18n'
import { bytes, duration, when } from '@/ui/format'
import { PageHead } from '@/ui/PageHead'
import { useLoad } from '@/ui/useLoad'
import type { RunStatus, ServerInfo } from '@/admin/types'
import { ErrorNote } from '@/ui/ErrorNote'
import { ADMIN_API } from '@/paths'
import { useRun } from '@/projects/useRun'
import { useToast } from '@/ui/toast'

function RunRow({ label, run }: { label: string; run: RunStatus | null }) {
  if (!run) {
    return (
      <tr>
        <td>{label}</td>
        <td colSpan={3} className="muted">
          {t('server.notYet')}
        </td>
        <td />
      </tr>
    )
  }

  return (
    <>
      <tr>
        <td>
          {label}
          <br />
          <small className="muted">{when(run.at)}</small>
        </td>
        <td>{t('server.verified', { n: run.databases })}</td>
        <td>
          <span className={`pill ${run.ok ? 'ok' : 'bad'}`}>{run.ok ? t('server.passed') : t('server.failed')}</span>
        </td>
        <td className="num">{run.bytes ? bytes(run.bytes) : '—'}</td>
        <td className="mono muted">{run.snapshot ?? ''}</td>
      </tr>
      {!run.ok && run.problems.length > 0 && (
        <tr>
          <td colSpan={5}>
            <ul className="m-0 list-disc pl-5 text-bad">
              {run.problems.slice(0, 5).map((p) => (
                <li key={p}>{p}</li>
              ))}
            </ul>
          </td>
        </tr>
      )}
    </>
  )
}

function EmailBox({ s, onSent }: { s: ServerInfo; onSent: () => void }) {
  const { error, run } = useRun(onSent)
  const toast = useToast()
  const q = s.email_queue
  const server = s.email_server
  const test = () =>
    void run(async () => {
      const r = await api<{ to: string }>('POST', `${ADMIN_API}/email/test`)
      toast(t('server.emailTestSent', { email: r.to }))
    })

  return (
    <section className="box">
      <h2>{t('server.email')}</h2>
      <div className="row">
        <span className={`pill ${server ? 'ok' : ''}`}>{t(server ? 'server.emailConfigured' : 'server.emailNotSet')}</span>
        <span className="muted grow">{server ? t('server.emailSendsAs', { host: server.host, port: server.port, from: server.from }) : t('server.emailOff')}</span>
        {server && (
          <button type="button" className="btn sm" onClick={test}>
            {t('server.emailTest')}
          </button>
        )}
      </div>
      <ErrorNote message={error} />
      <p className="hint" style={{ marginTop: 10 }}>
        {t('server.emailHint')}
      </p>
      {server && (
        <small className="muted" style={{ display: 'block' }}>
          {t('server.emailStats', { sent: q.sent_24h, failed: q.failed_24h, pending: q.pending })}
        </small>
      )}
      {q.last_error && (
        <small className="text-bad" style={{ display: 'block' }}>
          {t('server.emailLastError', { when: when(q.last_error.at), error: q.last_error.error })}
        </small>
      )}
    </section>
  )
}

export function Server() {
  const { data: s, error, reload } = useLoad(() => api<ServerInfo>('GET', `${ADMIN_API}/server`), 'server')
  const last = s?.last_backup

  return (
    <>
      <PageHead title={t('rail.server')} lead={s?.public_url.replace(/^https?:\/\//, '') ?? ''} />
      <ErrorNote message={error} />
      {s && (
        <>
          <div className="stats">
            <div className="stat">
              <small>{t('server.version')}</small>
              <b className="mono">{s.version}</b>
            </div>
            <div className="stat">
              <small>{t('server.uptime')}</small>
              <b>{duration(s.uptime_secs)}</b>
            </div>
            <div className="stat">
              <small>{t('server.data')}</small>
              <b>{bytes(s.data_bytes)}</b>
            </div>
            <div className="stat">
              <small>{t('server.lastBackup')}</small>
              <b className={last ? (last.ok ? 'text-ok' : 'text-bad') : 'text-muted'}>
                {last
                  ? t(last.ok ? 'server.backupOk' : 'server.backupFailed', { when: when(last.at) })
                  : t('server.noBackup')}
              </b>
            </div>
          </div>
          <EmailBox s={s} onSent={reload} />
          <section className="box">
            <h2>{t('server.backups')}</h2>
            <div className="tw">
              <table>
                <thead>
                  <tr>
                    <th>{t('server.col.run')}</th>
                    <th>{t('server.col.databases')}</th>
                    <th>{t('server.col.check')}</th>
                    <th className="num">{t('server.col.size')}</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  <RunRow label={t('server.run.backup')} run={s.last_backup} />
                  <RunRow label={t('server.run.restore')} run={s.last_restore_test} />
                </tbody>
              </table>
            </div>
            <p className="hint" style={{ marginTop: 12 }}>
              {t('server.backupsHint')}
            </p>
          </section>
        </>
      )}
    </>
  )
}
