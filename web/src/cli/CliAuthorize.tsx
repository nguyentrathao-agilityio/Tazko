import { useState, type FormEvent } from 'react'
import { api } from '@/http'
import { t } from '@/i18n'
import { AuthCard, Note } from '@/auth/AuthCard'
import { CLI_API } from '@/paths'
import { useRun } from '@/projects/useRun'
import { ErrorNote } from '@/ui/ErrorNote'
import type { Go } from '@/ui/Link'

type Device = { client: string; status: 'waiting' | 'approved' | 'denied' | 'expired'; email: string }

const FINISHED = {
  approved: ['cli.doneTitle', 'cli.doneLead'],
  denied: ['cli.deniedTitle', 'cli.deniedLead'],
} as const

const tidy = (raw: string) => {
  const letters = raw.toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, 8)
  return letters.length > 4 ? `${letters.slice(0, 4)}-${letters.slice(4)}` : letters
}

export function CliAuthorize({ go }: { go: Go }) {
  const [code, setCode] = useState('')
  const [device, setDevice] = useState<Device | null>(null)
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
        const found = await api<Device>('GET', path)
        setDevice(found.status === 'waiting' ? found : { ...found, status: 'expired' })
      } catch {
        throw new Error(t('cli.unknownCode'))
      }
    })
  }
  const decide = (approve: boolean) =>
    void run(async () => {
      await api('POST', `${path}/${approve ? 'approve' : 'deny'}`)
      setDevice((d) => d && { ...d, status: approve ? 'approved' : 'denied' })
    })
  const status = device?.status

  return (
    <AuthCard>
      {status === 'approved' || status === 'denied' ? (
        <>
          <h1 className="text-2xl font-bold">{t(FINISHED[status][0])}</h1>
          <p className="mt-2 text-muted">{t(FINISHED[status][1])}</p>
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
          <div className="row mt-4 justify-center">
            <button type="button" className="btn primary" onClick={() => decide(true)}>
              {t('cli.authorize')}
            </button>
            <button type="button" className="btn" onClick={() => decide(false)}>
              {t('common.cancel')}
            </button>
          </div>
          <p className="hint mt-3">
            {t('cli.signedInAs', { email: device.email })}
          </p>
        </>
      ) : (
        <>
          <h1 className="text-2xl font-bold">{t('cli.enterTitle')}</h1>
          <p className="mt-2 text-muted">{status === 'expired' ? t('cli.expired') : t('cli.enterLead')}</p>
          <form onSubmit={look} noValidate>
            <input
              className="codebig w-full border-0"
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
            <button type="submit" className="btn primary mt-2">
              {t('cli.continue')}
            </button>
          </form>
        </>
      )}
      <ErrorNote message={error} />
      <button type="button" className="btn sm mt-4" onClick={() => go('/')}>
        {t('cli.backToTazko')}
      </button>
    </AuthCard>
  )
}
