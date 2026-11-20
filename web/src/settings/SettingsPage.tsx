import { useEffect, useState } from 'react'
import type { Company, Me } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { when } from '@/ui/format'
import { TokenTable } from '@/tokens/TokenTable'
import { Icon } from '@/ui/Icon'
import type { Go } from '@/ui/Link'
import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import { useToast } from '@/ui/toast'
import { useLoad } from '@/ui/useLoad'
import { useRun } from '@/projects/useRun'
import { Tools } from '@/ui/Tools'
import { saveSettings, useSettings, type Settings } from '@/settings/store'
import { companyRoleLabel, isOwner } from '@/roles'
import { ErrorNote } from '@/ui/ErrorNote'
import { SwitchRow } from '@/ui/SwitchRow'
import { companyRoute, ME_API } from '@/paths'

export type SettingsTab = 'profile' | 'appearance' | 'tokens' | 'sessions'

const zones = [
  'UTC',
  'Asia/Ho_Chi_Minh',
  'Asia/Bangkok',
  'Asia/Singapore',
  'Asia/Tokyo',
  'Australia/Sydney',
  'Europe/London',
  'Europe/Berlin',
  'America/New_York',
  'America/Chicago',
  'America/Los_Angeles',
]

function Seg<K extends keyof Settings>({ name, label, options }: { name: K; label: string; options: [Settings[K], string][] }) {
  const s = useSettings()
  return (
    <div className="seg" role="radiogroup" aria-label={label}>
      {options.map(([value, text]) => (
        <button key={String(value)} type="button" role="radio" aria-checked={s[name] === value} onClick={() => void save({ [name]: value } as Partial<Settings>)}>
          {text}
        </button>
      ))}
    </div>
  )
}

function Switch({ name, label, hint }: { name: 'reduce_motion' | 'zen' | 'zen_quiet'; label: string; hint: string }) {
  const s = useSettings()
  return <SwitchRow label={label} hint={hint} on={s[name]} onChange={(on) => void save({ [name]: on })} />
}

const failedEvent = 'tazko:settings-failed'

async function save(change: Partial<Settings>) {
  try {
    await saveSettings(change)
  } catch {
    window.dispatchEvent(new Event(failedEvent))
  }
}

export function SettingsPage({ me, company, tab, go, onProfile }: { me: Me; company: Company; tab: SettingsTab; go: Go; onProfile: () => void }) {
  const [problem, setProblem] = useState<string | null>(null)
  useEffect(() => {
    const onFail = () => setProblem(t('settings.saveFailed'))
    window.addEventListener(failedEvent, onFail)

    return () => window.removeEventListener(failedEvent, onFail)
  }, [])
  const base = `${companyRoute(company.slug)}/me`
  const tabs: [SettingsTab, string][] = [
    ['profile', t('settings.tab.profile')],
    ['appearance', t('settings.tab.appearance')],
    ['tokens', t('settings.tab.tokens')],
    ['sessions', t('settings.tab.sessions')],
  ]

  return (
    <>
      <div className="head">
        <div>
          <h1>{t('settings.title')}</h1>
          <p className="muted">{t('settings.lead', { name: displayName(me.name, me.email) })}</p>
        </div>
        <span className="sp" />
        <Tools />
      </div>
      <div className="tabs2" role="tablist">
        {tabs.map(([k, label]) => (
          <button key={k} type="button" role="tab" aria-selected={tab === k} className={tab === k ? 'on' : ''} onClick={() => go(k === 'profile' ? base : `${base}/${k}`)}>
            {label}
          </button>
        ))}
      </div>
      <ErrorNote message={problem} />
      {tab === 'profile' && <Profile me={me} onSaved={onProfile} />}
      {tab === 'appearance' && <Appearance />}
      {tab === 'tokens' && (
        <TokenTable
          path={`${ME_API}/tokens`}
          scopes={isOwner(company.role) ? undefined : ['read', 'write']}
          title={t('tokens.personalTitle')}
          lead={t('tokens.personalLead')}
          footer={<p className="hint" style={{ marginTop: 10 }}>{t('tokens.personalHint')}</p>}
        />
      )}
      {tab === 'sessions' && <Sessions />}
    </>
  )
}

function Profile({ me, onSaved }: { me: Me; onSaved: () => void }) {
  const s = useSettings()
  const [name, setName] = useState(me.name)
  const [title, setTitle] = useState(s.job_title)
  const toast = useToast()
  const { error, run } = useRun()
  const shown = displayName(name, me.email)

  const submit = () =>
    void run(async () => {
      await api('PATCH', `${ME_API}/profile`, { name })
      await saveSettings({ job_title: title.trim() })
    }).then((ok) => {
      if (ok) {
        toast(t('settings.profileSaved'))
        onSaved()
      }
    })

  return (
    <>
      <section className="box">
        <h2>{t('settings.profile')}</h2>
        <ErrorNote message={error} />
        <div className="row" style={{ gap: 16, flexWrap: 'nowrap', marginBottom: 8 }}>
          <Avatar name={shown} big />
          <div>
            <b>{shown}</b>
            <div className="muted">{me.email}</div>
          </div>
        </div>
        <div className="field">
          <label htmlFor="pf-n">{t('settings.name')}</label>
          <input type="text" id="pf-n" value={name} onChange={(e) => setName(e.target.value)} />
        </div>
        <div className="field">
          <label htmlFor="pf-t">{t('settings.jobTitle')}</label>
          <input type="text" id="pf-t" value={title} onChange={(e) => setTitle(e.target.value)} placeholder={t('settings.jobTitleHint')} />
        </div>
        <div className="field">
          <label htmlFor="pf-e">{t('members.email')}</label>
          <input type="text" id="pf-e" value={me.email} disabled />
          <small className="hint">{t('settings.emailHint')}</small>
        </div>
        <div className="field">
          <label>{t('settings.photo')}</label>
          <Seg
            name="avatar"
            label={t('settings.photo')}
            options={[
              ['google', t('settings.photoGoogle')],
              ['initials', t('settings.photoInitials')],
            ]}
          />
          <small className="hint">{t('settings.photoHint')}</small>
        </div>
        <div className="field">
          <label htmlFor="pf-l">{t('settings.language')}</label>
          <select id="pf-l" style={{ maxWidth: 260 }} value="en" onChange={() => undefined}>
            <option value="en">English</option>
            <option disabled>{t('settings.vietnameseLater')}</option>
          </select>
        </div>
        <button type="button" className="btn primary" onClick={submit}>
          {t('settings.saveProfile')}
        </button>
      </section>
      <section className="box">
        <h2>{t('settings.companies')}</h2>
        <div className="tw">
          <table>
            <tbody>
              {me.companies.map((c) => (
                <tr key={c.slug}>
                  <td>
                    <div className="who">
                      <Avatar name={c.name} square />
                      <b>{c.name}</b>
                    </div>
                  </td>
                  <td>
                    <span className={`pill${isOwner(c.role) ? ' accent' : ''}`}>{t(companyRoleLabel(c.role))}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </>
  )
}

function now(tz: string, hour12: boolean): string {
  try {
    return new Date().toLocaleString('en-US', { timeZone: tz, weekday: 'short', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit', hour12 })
  } catch {
    return ''
  }
}

function Appearance() {
  const s = useSettings()
  const toast = useToast()
  const detect = () => {
    const zone = Intl.DateTimeFormat().resolvedOptions().timeZone
    if (!zone) return
    void save({ time_zone: zone }).then(() => toast(t('settings.zoneSet', { zone })))
  }
  const zoneList = zones.includes(s.time_zone) ? zones : [s.time_zone, ...zones]

  return (
    <>
      <section className="box">
        <h2>{t('settings.theme')}</h2>
        <div className="themes" role="radiogroup" aria-label={t('settings.theme')}>
          {(['system', 'light', 'dark'] as const).map((k) => (
            <button key={k} type="button" className="thm" role="radio" aria-checked={s.theme === k} onClick={() => void save({ theme: k })}>
              <span className={`tp ${k}`}>
                <i />
                <i />
                <i />
              </span>
              {t(`settings.theme.${k}`)}
            </button>
          ))}
        </div>
        <p className="hint" style={{ padding: '8px 0 0', border: 0 }}>
          {t('settings.themeHint')}
        </p>
      </section>
      <section className="box">
        <h2>{t('settings.size')}</h2>
        <div className="field">
          <label htmlFor="zm">
            {t('settings.zoom')} <output htmlFor="zm">{s.zoom}%</output>
          </label>
          <div className="row" style={{ flexWrap: 'nowrap' }}>
            <input
              type="range"
              id="zm"
              min={80}
              max={150}
              step={10}
              value={s.zoom}
              onChange={(e) => void save({ zoom: Number(e.target.value) })}
              style={{ flex: 1, maxWidth: 340 }}
            />
            <button type="button" className="btn sm" onClick={() => void save({ zoom: 100 })}>
              {t('settings.reset')}
            </button>
          </div>
          <small className="hint">{t('settings.zoomHint')}</small>
        </div>
        <div className="field">
          <label>{t('settings.density')}</label>
          <Seg
            name="density"
            label={t('settings.density')}
            options={[
              ['comfortable', t('zen.comfortable')],
              ['compact', t('zen.compact')],
            ]}
          />
        </div>
        <SwitchRow
          label={t('settings.labelBars')}
          hint={t('settings.labelBarsHint')}
          on={s.label_style === 'bars'}
          onChange={(on) => void save({ label_style: on ? 'bars' : 'names' })}
        />
      </section>
      <section className="box">
        <h2>{t('settings.zen')}</h2>
        <p className="muted" style={{ margin: '0 0 12px' }}>
          {t('settings.zenLead1')} <span className="kbd">Z</span> {t('settings.zenLead2')} <span className="kbd">Z</span> {t('settings.zenLead3')}{' '}
          <span className="kbd">Esc</span> {t('settings.zenLead4')}
        </p>
        <Switch name="zen" label={t('settings.zenOn')} hint={t('settings.zenOnHint')} />
        <div className="field">
          <label>{t('settings.zenDensity')}</label>
          <Seg
            name="zen_density"
            label={t('settings.zenDensity')}
            options={[
              ['comfortable', t('zen.comfortable')],
              ['compact', t('zen.compact')],
              ['minimal', t('zen.minimal')],
            ]}
          />
        </div>
        <Switch name="zen_quiet" label={t('settings.zenQuiet')} hint={t('settings.zenQuietHint')} />
      </section>
      <section className="box">
        <h2>{t('settings.dateTime')}</h2>
        <div className="field">
          <label htmlFor="tz">{t('settings.timeZone')}</label>
          <div className="row" style={{ flexWrap: 'nowrap' }}>
            <select id="tz" style={{ maxWidth: 300 }} value={s.time_zone} onChange={(e) => void save({ time_zone: e.target.value })}>
              {zoneList.map((z) => (
                <option key={z} value={z}>
                  {z.replace(/_/g, ' ')}
                </option>
              ))}
            </select>
            <button type="button" className="btn sm" onClick={detect}>
              {t('settings.detect')}
            </button>
          </div>
          <small className="hint">{t('settings.nowIs', { now: now(s.time_zone, s.time_format === '12') })}</small>
        </div>
        <div className="field">
          <label htmlFor="df">{t('settings.dateFormat')}</label>
          <select id="df" style={{ maxWidth: 260 }} value={s.date_format} onChange={(e) => void save({ date_format: e.target.value as Settings['date_format'] })}>
            <option value="MDY">Nov 7</option>
            <option value="DMY">7 Nov</option>
            <option value="ISO">2026-11-07</option>
          </select>
        </div>
        <div className="field">
          <label>{t('settings.timeFormat')}</label>
          <Seg
            name="time_format"
            label={t('settings.timeFormat')}
            options={[
              ['12', t('settings.h12')],
              ['24', t('settings.h24')],
            ]}
          />
        </div>
        <div className="field">
          <label htmlFor="wk">{t('settings.weekStart')}</label>
          <select id="wk" style={{ maxWidth: 260 }} value={s.week_start} onChange={(e) => void save({ week_start: e.target.value as Settings['week_start'] })}>
            <option value="mon">{t('settings.monday')}</option>
            <option value="sun">{t('settings.sunday')}</option>
            <option value="sat">{t('settings.saturday')}</option>
          </select>
        </div>
      </section>
      <section className="box">
        <h2>{t('settings.accessibility')}</h2>
        <Switch name="reduce_motion" label={t('settings.reduceMotion')} hint={t('settings.reduceMotionHint')} />
      </section>
    </>
  )
}

type SessionRow = { id: string; user_agent: string | null; created_at: number; last_used_at: number; current: boolean }

function device(ua: string | null): string {
  if (!ua) return t('settings.unknownDevice')
  const browser = /Edg\//.test(ua) ? 'Edge' : /Chrome\//.test(ua) ? 'Chrome' : /Firefox\//.test(ua) ? 'Firefox' : /Safari\//.test(ua) ? 'Safari' : t('settings.browser')
  const os = /Windows/.test(ua) ? 'Windows' : /Mac OS X/.test(ua) ? 'macOS' : /Android/.test(ua) ? 'Android' : /iPhone|iPad/.test(ua) ? 'iOS' : /Linux/.test(ua) ? 'Linux' : ''
  return os ? `${browser} on ${os}` : browser
}

function Sessions() {
  const path = `${ME_API}/sessions`
  const { data, reload } = useLoad(() => api<{ sessions: SessionRow[] }>('GET', path), path)
  const { error, runAndToast } = useRun(reload)

  return (
    <section className="box">
      <div className="row">
        <div className="grow">
          <h2 style={{ margin: 0 }}>{t('settings.sessionsTitle')}</h2>
          <p className="muted" style={{ margin: '4px 0 0' }}>
            {t('settings.sessionsLead')}
          </p>
        </div>
        <button
          type="button"
          className="btn"
          onClick={() => void runAndToast(() => api('POST', `${path}/others`), t('settings.othersEnded'))}
        >
          {t('settings.endOthers')}
        </button>
      </div>
      <ErrorNote message={error} />
      <div className="tw" style={{ marginTop: 14 }}>
        <table>
          <tbody>
            {data?.sessions.map((row) => (
              <tr key={row.id}>
                <td>
                  <div className="who">
                    <span className="av sq c0">
                      <Icon name="laptop" />
                    </span>
                    <div>
                      <b>{device(row.user_agent)}</b>
                      <small>{t('settings.lastActive', { when: when(row.last_used_at) })}</small>
                    </div>
                  </div>
                </td>
                <td className="num">
                  {row.current ? (
                    <span className="pill ok">{t('settings.thisSession')}</span>
                  ) : (
                    <button
                      type="button"
                      className="btn sm"
                      onClick={() => void runAndToast(() => api('DELETE', `${path}/${row.id}`), t('settings.ended', { device: device(row.user_agent) }))}
                    >
                      {t('home.signOut')}
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}
