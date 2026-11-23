import { useState } from 'react'
import { api } from '@/http'
import { t, type MessageKey } from '@/i18n'
import { ME_API } from '@/paths'
import { useRun } from '@/projects/useRun'
import { useSettings, type Settings } from '@/settings/store'
import { ErrorNote } from '@/ui/ErrorNote'
import { Modal } from '@/ui/Modal'
import { Switch } from '@/ui/SwitchRow'
import { useToast } from '@/ui/toast'

type Save = (change: Partial<Settings>) => Promise<void>
type Kind = 'mention' | 'assigned' | 'card_added' | 'due'
type Preview = { kind: string; subject: string; to: string; html: string }

const KINDS: Kind[] = ['mention', 'assigned', 'card_added', 'due']
const FREQUENCIES: Settings['email_frequency'][] = ['instant', 'hourly', 'daily', 'never']
const REMINDERS: Settings['due_reminder'][] = ['at', '1h', '1d', '2d']

const eightLocalInUtc = () => {
  const d = new Date()
  d.setHours(8, 0, 0, 0)
  return d.getUTCHours()
}

export function Notifications({ emailOn, save }: { emailOn: boolean; save: Save }) {
  const s = useSettings()
  const toast = useToast()
  const { error, run } = useRun()
  const [previews, setPreviews] = useState<{ from: string | null; previews: Preview[] } | null>(null)
  const [shown, setShown] = useState(0)
  const app = (k: Kind) => k === 'due' || s[`notify_${k}_app` as keyof Settings] === true
  const email = (k: Kind) => s[`notify_${k}_email` as keyof Settings] === true
  const setFrequency = (f: Settings['email_frequency']) => void save(f === 'daily' ? { email_frequency: f, daily_hour_utc: eightLocalInUtc() } : { email_frequency: f })
  const openPreviews = () =>
    void run(async () => {
      setShown(0)
      setPreviews(await api<{ from: string | null; previews: Preview[] }>('GET', `${ME_API}/email/previews`))
    })
  const test = () =>
    void run(async () => {
      const r = await api<{ to: string }>('POST', `${ME_API}/email/test`)
      toast(t('notif.testSent', { email: r.to }))
    })
  const current = previews?.previews[shown]

  return (
    <>
      <section className="box">
        <h2>{t('notif.what')}</h2>
        {!emailOn && <p className="hint">{t('notif.emailOff')}</p>}
        <div className="tw" style={{ marginTop: 10 }}>
          <table>
            <thead>
              <tr>
                <th>{t('notif.event')}</th>
                <th>{t('notif.inApp')}</th>
                <th>{t('notif.byEmail')}</th>
              </tr>
            </thead>
            <tbody>
              {KINDS.map((k) => {
                const label = t(`notif.kind.${k}` as MessageKey)
                return (
                  <tr key={k}>
                    <td>{label}</td>
                    <td>
                      <Switch label={`${label}: ${t('notif.inApp')}`} on={app(k)} disabled={k === 'due'} onChange={(on) => void save({ [`notify_${k}_app`]: on } as Partial<Settings>)} />
                    </td>
                    <td>
                      <Switch label={`${label}: ${t('notif.byEmail')}`} on={emailOn && email(k)} disabled={!emailOn} onChange={(on) => void save({ [`notify_${k}_email`]: on } as Partial<Settings>)} />
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
        <p className="hint" style={{ marginTop: 10 }}>
          {t('notif.alwaysInApp')}
        </p>
      </section>
      <section className="box">
        <h2>{t('notif.delivery')}</h2>
        <div className="field">
          <label htmlFor="n-fr">{t('notif.send')}</label>
          <select id="n-fr" style={{ maxWidth: 300 }} value={s.email_frequency} disabled={!emailOn} onChange={(e) => setFrequency(e.target.value as Settings['email_frequency'])}>
            {FREQUENCIES.map((f) => (
              <option key={f} value={f}>
                {t(`notif.freq.${f}` as MessageKey)}
              </option>
            ))}
          </select>
          <small className="hint">{t('notif.instantHint')}</small>
        </div>
        <div className="field">
          <label htmlFor="n-rm">{t('notif.reminder')}</label>
          <select id="n-rm" style={{ maxWidth: 300 }} value={s.due_reminder} onChange={(e) => void save({ due_reminder: e.target.value as Settings['due_reminder'] })}>
            {REMINDERS.map((r) => (
              <option key={r} value={r}>
                {t(`notif.remind.${r}` as MessageKey)}
              </option>
            ))}
          </select>
          <small className="hint">{t('notif.reminderHint')}</small>
        </div>
        <ErrorNote message={error} />
        <div className="row">
          <button type="button" className="btn" onClick={openPreviews}>
            {t('notif.preview')}
          </button>
          {emailOn && (
            <button type="button" className="btn" onClick={test}>
              {t('notif.test')}
            </button>
          )}
        </div>
      </section>
      {previews && current && (
        <Modal label={t('notif.previewTitle')} onClose={() => setPreviews(null)}>
          <h2>{t('notif.previewTitle')}</h2>
          <div className="tabs2" role="tablist">
            {previews.previews.map((p, i) => (
              <button key={p.kind} type="button" role="tab" aria-selected={i === shown} className={i === shown ? 'on' : ''} onClick={() => setShown(i)}>
                {t(`notif.preview.${p.kind}` as MessageKey)}
              </button>
            ))}
          </div>
          <div className="mailp">
            <div className="mailh">
              {previews.from && (
                <div>
                  <small className="muted">{t('notif.from')}</small> {previews.from}
                </div>
              )}
              <div>
                <small className="muted">{t('notif.to')}</small> {current.to}
              </div>
              <div>
                <small className="muted">{t('notif.subject')}</small> <b>{current.subject}</b>
              </div>
            </div>
            <iframe title={current.subject} sandbox="" srcDoc={current.html} />
          </div>
          <div className="acts">
            <button type="button" className="btn primary" onClick={() => setPreviews(null)}>
              {t('notif.close')}
            </button>
          </div>
        </Modal>
      )}
    </>
  )
}
