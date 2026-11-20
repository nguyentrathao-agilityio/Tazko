import { useState, type FormEvent } from 'react'
import { api } from '@/http'
import { t } from '@/i18n'
import { useRun } from '@/projects/useRun'
import { ErrorNote } from '@/ui/ErrorNote'
import { when } from '@/ui/format'
import { Icon } from '@/ui/Icon'
import { Modal } from '@/ui/Modal'
import { useLoad } from '@/ui/useLoad'
import { SecretModal } from '@/tokens/SecretModal'
import { Deliveries } from '@/webhooks/Deliveries'
import { health, type Webhook } from '@/webhooks/types'

type Project = { id: number; name: string }

const PICKED = new Set(['card.created', 'card.moved'])

const PILL = { paused: '', healthy: 'ok', failing: 'bad' } as const

export function WebhookList({ path, projects }: { path: string; projects: Project[] }) {
  const { data, error: loadError, reload } = useLoad(() => api<{ webhooks: Webhook[]; events: string[] }>('GET', path), path)
  const { error, run } = useRun(reload)
  const creating = useRun(reload)
  const [making, setMaking] = useState(false)
  const [busy, setBusy] = useState(false)
  const [secret, setSecret] = useState<string | null>(null)
  const [open, setOpen] = useState<number | null>(null)
  const close = () => {
    setMaking(false)
    creating.setError(null)
  }

  const create = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const f = new FormData(e.currentTarget)
    const url = String(f.get('url') ?? '').trim()
    if (!url) {
      e.currentTarget.querySelector<HTMLInputElement>('#hk-u')?.focus()
      return
    }

    const events = f.getAll('events').map(String)
    if (events.length === 0) {
      creating.setError(t('webhooks.pickEvent'))
      return
    }

    if (busy) return
    const pick = String(f.get('projects') ?? '')
    setBusy(true)
    void creating
      .run(async () => {
        const r = await api<{ id: number; secret: string }>('POST', path, { url, events, projects: pick ? [Number(pick)] : undefined })
        setMaking(false)
        setOpen(r.id)
        setSecret(r.secret)
      })
      .finally(() => setBusy(false))
  }

  return (
    <section className="box">
      <div className="row">
        <div className="grow">
          <h2 style={{ margin: 0 }}>{t('webhooks.title')}</h2>
          <p className="muted" style={{ margin: '4px 0 0' }}>
            {t('webhooks.lead')}
          </p>
        </div>
        <button type="button" className="btn primary" onClick={() => setMaking(true)}>
          <Icon name="plus" />
          {t('webhooks.add')}
        </button>
      </div>
      <ErrorNote message={error ?? loadError} />
      <div style={{ marginTop: 12 }}>
        {data?.webhooks.map((h) => (
          <div className="hook" key={h.id}>
            <div className="row hookr">
              <span className={`pill ${PILL[health(h)]}`}>{t(`webhooks.${health(h)}`)}</span>
              <div className="grow hooku">
                <b>{h.url}</b>
                <small className="muted">
                  {t('webhooks.summary', {
                    n: h.events.length,
                    projects: h.project_names ? h.project_names.join(', ') : t('webhooks.allProjects'),
                    last: h.last.at ? when(h.last.at) : t('webhooks.never'),
                  })}
                </small>
              </div>
              <button
                type="button"
                className="switchbtn"
                role="switch"
                aria-checked={h.active}
                aria-label={t('webhooks.active')}
                onClick={() => void run(() => api('PATCH', `${path}/${h.id}`, { active: !h.active }))}
              />
              <button type="button" className="btn sm" aria-expanded={open === h.id} onClick={() => setOpen(open === h.id ? null : h.id)}>
                {t(open === h.id ? 'webhooks.hide' : 'webhooks.deliveries')}
              </button>
            </div>
            {open === h.id && <Deliveries path={path} hook={h} onRemoved={reload} />}
          </div>
        ))}
        {data?.webhooks.length === 0 && <p className="muted">{t('webhooks.none')}</p>}
      </div>
      {making && data && (
        <Modal label={t('webhooks.add')} onClose={close}>
          <h2>{t('webhooks.add')}</h2>
          <form onSubmit={create} noValidate>
            <div className="field">
              <label htmlFor="hk-u">{t('webhooks.address')}</label>
              <input type="text" inputMode="url" id="hk-u" name="url" placeholder="https://example.com/hooks/tazko" maxLength={500} required />
            </div>
            <div className="field">
              <label htmlFor="hk-p">{t('webhooks.projects')}</label>
              <select id="hk-p" name="projects" style={{ width: '100%' }}>
                <option value="">{t('webhooks.allMyProjects')}</option>
                {projects.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
              <small className="hint">{t('webhooks.projectsHint')}</small>
            </div>
            <fieldset className="field">
              <legend>{t('webhooks.events')}</legend>
              {data.events.map((e) => (
                <label className="opt" key={e}>
                  <input type="checkbox" name="events" value={e} defaultChecked={PICKED.has(e)} />
                  <span className="mono">{e}</span>
                </label>
              ))}
            </fieldset>
            <ErrorNote message={creating.error} />
            <div className="acts">
              <button type="button" className="btn" onClick={close}>
                {t('common.cancel')}
              </button>
              <button type="submit" className="btn primary" disabled={busy}>
                {t('webhooks.add')}
              </button>
            </div>
          </form>
        </Modal>
      )}
      {secret && <SecretModal title={t('webhooks.made')} lead={t('webhooks.madeLead')} label={t('webhooks.secret')} text={secret} onClose={() => setSecret(null)} />}
    </section>
  )
}
