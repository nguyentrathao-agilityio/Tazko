import { api } from '@/http'
import { t } from '@/i18n'
import { bytes, duration, when } from '@/ui/format'
import { useLoad } from '@/ui/useLoad'
import type { ServerInfo } from '@/admin/types'

export function Server() {
  const { data: s, error } = useLoad(() => api<ServerInfo>('GET', '/api/v1/admin/server'), 'server')

  return (
    <>
      <div className="head">
        <div>
          <h1>{t('rail.server')}</h1>
          <p className="muted">{s?.public_url.replace(/^https?:\/\//, '') ?? ''}</p>
        </div>
      </div>
      {error && <p className="error" role="alert">{error}</p>}
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
              <b style={{ color: s.last_backup ? 'var(--ok)' : 'var(--muted)' }}>
                {s.last_backup ? when(s.last_backup) : t('server.noBackup')}
              </b>
            </div>
          </div>
          <section className="box">
            <h2>{t('server.email')}</h2>
            <p className="muted">{s.email === 'on' ? t('server.emailOn') : t('server.emailOff')}</p>
          </section>
          <section className="box">
            <h2>{t('server.backups')}</h2>
            <p className="muted">{t('server.backupsNone')}</p>
          </section>
        </>
      )}
    </>
  )
}
