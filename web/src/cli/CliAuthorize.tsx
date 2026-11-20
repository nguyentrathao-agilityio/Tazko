import { useState, type FormEvent } from 'react'
import { api } from '@/http'
import { t } from '@/i18n'
import { AuthCard, Note } from '@/auth/AuthCard'
import { CLI_API } from '@/paths'
import { useRun } from '@/projects/useRun'
import { ErrorNote } from '@/ui/ErrorNote'
import type { Go } from '@/ui/Link'

type Device = { client: string; status: 'waiting' | 'approved' | 'denied' | 'expired'; email: string }

const tidy = (raw: string) => {
  const letters = raw.toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, 8)
  return letters.length > 4 ? `${letters.slice(0, 4)}-${letters.slice(4)}` : letters
}

export function CliAuthorize({ go }: { go: Go }) {
  const [code, setCode] = useState('')
  const [device, setDevice] = useState<Device | null>(null)
  const [done, setDone] = useState<'approved' | 'denied' | null>(null)
  const { error, setError, run } = useRun()
  const path = `${CLI_API}/device/${encodeURIComponent(code)}`
  const look = (e: FormEvent) => {
    e.preventDefault()
    if (code.length !== 9) {
      setError(t('cli.codeShape'))
      return
    }

    void run(async () => {
      try {
        setDevice(await api<Device>('GET', path))
      } catch {
        throw new Error(t('cli.unknownCode'))
      }
    })
  }
  const decide = (approve: boolean) =>
    void run(async () => {
      await api('POST', `${path}/${approve ? 'approve' : 'deny'}`)
      setDone(approve ? 'approved' : 'denied')
    })
  const status = done ?? device?.status

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
      ) : device && status === 'waiting' ? (
        <>
          <h1 className="text-2xl font-bold">{t('cli.title')}</h1>
          <p className="mt-2 text-muted">{t('cli.lead', { client: device.client || t('cli.unknownDevice') })}</p>
          <div className="codebig">{code}</div>
          <p className="hint text-center">{t('cli.onlyIfYou')}</p>
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
            {t('cli.signedInAs', { email: device.email })}
          </p>
        </>
      ) : (
        <>
          <h1 className="text-2xl font-bold">{t('cli.enterTitle')}</h1>
          <p className="mt-2 text-muted">{status === 'expired' ? t('cli.expired') : t('cli.enterLead')}</p>
          <form onSubmit={look} noValidate>
            <input
              className="codebig"
              style={{ width: '100%', border: 0 }}
              aria-label={t('cli.codeLabel')}
              placeholder="XXXX-XXXX"
              value={code}
              autoComplete="off"
              spellCheck={false}
              onChange={(e) => {
                setDevice(null)
                setCode(tidy(e.target.value))
              }}
            />
            <button type="submit" className="btn primary" style={{ marginTop: 8 }}>
              {t('cli.continue')}
            </button>
          </form>
        </>
      )}
      <ErrorNote message={error} />
      <button type="button" className="btn sm" style={{ marginTop: 16 }} onClick={() => go('/')}>
        {t('cli.backToTazko')}
      </button>
    </AuthCard>
  )
}
