import { useState, type FormEvent } from 'react'
import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { EmptyState } from '@/ui/EmptyState'
import { Link, type Go } from '@/ui/Link'
import { Modal } from '@/ui/Modal'
import { useToast } from '@/ui/toast'
import { Tools } from '@/ui/Tools'
import { useLoad } from '@/ui/useLoad'
import { background } from '@/projects/look'
import type { ProjectView } from '@/projects/types'
import { useRun } from '@/projects/useRun'
import type { BoardTemplate, TemplateList } from '@/templates/types'
import { boardRoute, companyApi, companyRoute, projectApi, projectRoute } from '@/paths'

type Tab = 'all' | 'co' | 'admin'

export function TemplatesPage({ company, project, go }: { company: Company; project: number; go: Go }) {
  const base = companyApi(company.slug)
  const projectBase = projectApi(company.slug, project)
  const { data: proj } = useLoad(() => api<ProjectView>('GET', projectBase), projectBase)
  const { data, error: loadError, reload } = useLoad(() => api<TemplateList>('GET', `${base}/templates`), `${base}/templates`)
  const { error, run } = useRun(reload)
  const toast = useToast()
  const [tab, setTab] = useState<Tab>('all')
  const [preview, setPreview] = useState<BoardTemplate | null>(null)
  const [using, setUsing] = useState<BoardTemplate | null>(null)
  const list = (data?.templates ?? []).filter((x) => tab === 'all' || (tab === 'admin' ? x.admin : !x.admin))
  const projectPath = projectRoute(company.slug, project)

  return (
    <>
      <div className="crumb">
        <Link to={companyRoute(company.slug)} go={go}>
          {t('nav.projects')}
        </Link>{' '}
        /{' '}
        <Link to={projectPath} go={go}>
          {proj?.name ?? '…'}
        </Link>{' '}
        / {t('tpl.title')}
      </div>
      <div className="head">
        <div>
          <h1>{t('tpl.heading')}</h1>
          <p className="muted" style={{ margin: 0 }}>
            {t('tpl.lead')}
          </p>
        </div>
        <span className="sp" />
        <Tools />
      </div>
      <div className="tabs2" role="tablist">
        {(
          [
            ['all', t('tpl.all')],
            ['co', data?.company ?? company.name],
            ['admin', t('tpl.offered')],
          ] as [Tab, string][]
        ).map(([k, label]) => (
          <button key={k} type="button" role="tab" aria-selected={tab === k} className={tab === k ? 'on' : ''} onClick={() => setTab(k)}>
            {label}
          </button>
        ))}
      </div>
      {(error || loadError) && (
        <p className="error" role="alert">
          {error ?? loadError}
        </p>
      )}
      {data && list.length === 0 && <EmptyState title={t('tpl.noneTitle')} body={t('tpl.noneBody')} />}
      <div className="tgrid">
        {list.map((x) => {
          const cards = x.lists.reduce((n, l) => n + l.cards.length, 0)
          return (
            <article key={x.id} className="tcard" aria-label={x.name}>
              <div className="tprev" style={{ background: background(x.background) }} aria-hidden="true">
                {x.lists.slice(0, 5).map((l, i) => (
                  <i key={i}>
                    <s />
                    {l.cards.slice(0, 3).map((_, j) => (
                      <s key={j} />
                    ))}
                  </i>
                ))}
              </div>
              <div className="tb">
                <div className="row" style={{ flexWrap: 'nowrap' }}>
                  <h3 style={{ flex: 1 }}>{x.name}</h3>
                  {x.admin && <span className="pill accent">{t('tpl.adminPill')}</span>}
                </div>
                {x.description && <p>{x.description}</p>}
                <small className="muted">{t('tpl.counts', { lists: x.lists.length, cards, by: x.by })}</small>
                <div className="row">
                  <button type="button" className="btn sm" onClick={() => setPreview(x)}>
                    {t('tpl.preview')}
                  </button>
                  {proj?.can.create_board && (
                    <button type="button" className="btn sm primary" onClick={() => setUsing(x)}>
                      {t('tpl.use')}
                    </button>
                  )}
                  {data?.can_offer && !x.admin && (
                    <button
                      type="button"
                      className="btn sm quiet"
                      onClick={() => void run(() => api('PUT', `${base}/templates/${x.id.slice(2)}/offer`)).then((ok) => ok && toast(t('tpl.offeredToast')))}
                    >
                      {t('tpl.offer')}
                    </button>
                  )}
                  {data?.can_offer && x.admin && (
                    <button type="button" className="btn sm quiet" onClick={() => void run(() => api('DELETE', `${base}/admin-templates/${x.id.slice(2)}`))}>
                      {t('tpl.withdraw')}
                    </button>
                  )}
                </div>
              </div>
            </article>
          )
        })}
      </div>
      {preview && (
        <Modal label={preview.name} onClose={() => setPreview(null)}>
          <h2>{preview.name}</h2>
          {preview.description && <p className="muted">{preview.description}</p>}
          {preview.lists.map((l, i) => (
            <div key={i} className="box" style={{ padding: '10px 14px', margin: '8px 0' }}>
              <b>{l.title}</b>
              {l.cards.length ? (
                <ul style={{ margin: '.3em 0', paddingLeft: '1.1em' }}>
                  {l.cards.map((c, j) => (
                    <li key={j}>{c}</li>
                  ))}
                </ul>
              ) : (
                <div className="muted" style={{ fontSize: '.85rem' }}>
                  {t('tpl.noStarter')}
                </div>
              )}
            </div>
          ))}
          <div className="acts">
            <button type="button" className="btn" onClick={() => setPreview(null)}>
              {t('pop.close')}
            </button>
          </div>
        </Modal>
      )}
      {using && (
        <UseTemplate
          template={using}
          onClose={() => setUsing(null)}
          onCreate={async (name) => {
            const res = await api<{ id: number }>('POST', `${projectBase}/boards`, { name, template: using.id })
            setUsing(null)
            go(boardRoute(company.slug, res.id))
          }}
        />
      )}
    </>
  )
}

function UseTemplate({ template, onClose, onCreate }: { template: BoardTemplate; onClose: () => void; onCreate: (name: string) => Promise<void> }) {
  const [problem, setProblem] = useState<string | null>(null)
  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const name = String(new FormData(e.currentTarget).get('name') ?? '').trim()
    if (!name) {
      setProblem(t('board.titleNeeded'))
      return
    }

    onCreate(name).catch((err: unknown) => setProblem(err instanceof Error ? err.message : String(err)))
  }

  return (
    <Modal label={t('tpl.useTitle', { name: template.name })} onClose={onClose}>
      <h2>{t('tpl.useTitle', { name: template.name })}</h2>
      <form onSubmit={submit} noValidate>
        <div className="field">
          <label htmlFor="tu-n">{t('board.title')}</label>
          <input type="text" id="tu-n" name="name" defaultValue={template.name} />
          {problem && (
            <p className="error" role="alert">
              {problem}
            </p>
          )}
        </div>
        <div className="acts">
          <button type="button" className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" type="submit">
            {t('board.create')}
          </button>
        </div>
      </form>
    </Modal>
  )
}
