import { useState } from 'react'
import { api } from '@/http'
import { t } from '@/i18n'
import { AuthCard, Note } from '@/auth/AuthCard'
import { CLI_API } from '@/paths'
import { useRun } from '@/projects/useRun'
import { ErrorNote } from '@/ui/ErrorNote'
import { useLoad } from '@/ui/useLoad'
import type { Go } from '@/ui/Link'

type Device = { client: string; status: 'waiting' | 'approved' | 'denied' | 'expired'; email: string }

export function CliAuthorize({ search, go }: { search: string; go: Go }) {
  const code = (new URLSearchParams(search).get('code') ?? '').toUpperCase()
  const path = `${CLI_API}/device/${encodeURIComponent(code)}`
  const { data, error: loadError } = useLoad(() => (code ? api<Device>('GET', path) : Promise.reject(new Error(t('cli.noCode')))), path)
  const [done, setDone] = useState<'approved' | 'denied' | null>(null)
  const { error, run } = useRun()
  const decide = (approve: boolean) =>
    void run(async () => {
      await api('POST', `${path}/${approve ? 'approve' : 'deny'}`)
      setDone(approve ? 'approved' : 'denied')
    })
  const status = done ?? data?.status

  return (
    <AuthCard>
      {status === 'approved' ? (
        <>
          <h1 className="text-2xl font-bold">{t('cli.doneTitle')}</h1>
          <p className="mt-2 text-muted">{t('cli.doneLead')}</p>
        </>
      ) : status === 'denied' ? (
        <>
          <h1 className="text-2xl font-bold">{t('cli.deniedTitle')}</h1>
          <p className="mt-2 text-muted">{t('cli.deniedLead')}</p>
        </>
      ) : (
        <>
          <h1 className="text-2xl font-bold">{t('cli.title')}</h1>
          {data && status === 'waiting' && <p className="mt-2 text-muted">{t('cli.lead', { client: data.client || t('cli.unknownDevice') })}</p>}
          {status === 'expired' && <p className="mt-2 text-muted">{t('cli.expired')}</p>}
          <div className="codebig">{code || '—'}</div>
          {status === 'waiting' && (
            <>
              <p className="hint text-center">{t('cli.checkCode')}</p>
              <Note>
                <b>{t('cli.canTitle')}</b>
                <br />
                {t('cli.can1')}
                <br />
                {t('cli.can2')}
                <br />
                {t('cli.cannot')}
              </Note>
              <div className="row" style={{ marginTop: 16, justifyContent: 'center' }}>
                <button type="button" className="btn primary" onClick={() => decide(true)}>
                  {t('cli.authorize')}
                </button>
                <button type="button" className="btn" onClick={() => decide(false)}>
                  {t('common.cancel')}
                </button>
              </div>
              <p className="hint" style={{ marginTop: 12 }}>
                {t('cli.signedInAs', { email: data?.email ?? '' })}
              </p>
            </>
          )}
        </>
      )}
      <ErrorNote message={error ?? (loadError ? t('cli.unknownCode') : null)} />
      <button type="button" className="btn sm" style={{ marginTop: 16 }} onClick={() => go('/')}>
        {t('cli.backToTazko')}
      </button>
    </AuthCard>
  )
}
