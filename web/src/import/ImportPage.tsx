import { useRef, useState, type DragEvent } from 'react'
import type { Company } from '@/api'
import { api, errorMessage, sendJsonFile } from '@/http'
import { t, type MessageKey } from '@/i18n'
import { EmptyState } from '@/ui/EmptyState'
import { ErrorNote } from '@/ui/ErrorNote'
import { Link, type Go } from '@/ui/Link'
import { useLoad } from '@/ui/useLoad'
import type { ProjectView } from '@/projects/types'
import type { ImportPerson, ImportPreview, ImportResult } from '@/import/types'
import { boardRoute, importApi, importPreviewApi, projectApi, projectRoute, type ImportChoices } from '@/paths'

const MAX_FILE_BYTES = 50 * 1024 * 1024
const TAZKO_COLOURS = 8
const STEPS: MessageKey[] = ['import.step1', 'import.step2', 'import.step3']

type Loaded = { file: File; preview: ImportPreview }
type PeopleChoices = Record<string, number | null>

function suggested(p: ImportPerson): string {
  return p.match === null ? '' : String(p.match)
}

export function ImportPage({ company, project, go }: { company: Company; project: number; go: Go }) {
  const projectBase = projectApi(company.slug, project)
  const back = projectRoute(company.slug, project)
  const { data: proj, error: loadError } = useLoad(() => api<ProjectView>('GET', projectBase), projectBase)
  const [loaded, setLoaded] = useState<Loaded | null>(null)
  const [result, setResult] = useState<ImportResult | null>(null)
  const [reading, setReading] = useState(false)
  const [importing, setImporting] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const running = useRef(false)

  if (loadError)
    return (
      <EmptyState title={t('project.missingTitle')} body={t('project.missingBody')}>
        <Link to={back} go={go} className="btn">
          {t('import.back')}
        </Link>
      </EmptyState>
    )
  if (!proj)
    return (
      <p role="status" className="muted">
        {t('common.loading')}
      </p>
    )
  if (!proj.can.create_board)
    return (
      <EmptyState title={t('import.notAllowed')} body={t('import.notAllowedBody')}>
        <Link to={back} go={go} className="btn">
          {t('import.back')}
        </Link>
      </EmptyState>
    )

  const choose = (file: File | undefined) => {
    if (!file || reading) return
    if (file.size > MAX_FILE_BYTES) {
      setError(t('import.tooLarge'))
      return
    }

    setReading(true)
    setError(null)
    sendJsonFile<ImportPreview>(importPreviewApi(company.slug, project, file.name), file)
      .then(
        (preview) => setLoaded({ file, preview }),
        (e: unknown) => setError(errorMessage(e)),
      )
      .finally(() => setReading(false))
  }

  const start = (people: PeopleChoices, options: ImportChoices) => {
    if (!loaded || running.current) return
    running.current = true
    setImporting(true)
    setError(null)
    const head = JSON.stringify({ name: loaded.file.name, ...options, people })
    const body = new Blob([head.slice(0, -1), ',"file":', loaded.file, '}'], { type: 'application/json' })
    sendJsonFile<ImportResult>(importApi(company.slug, project), body)
      .then(setResult, (e: unknown) => setError(errorMessage(e)))
      .finally(() => {
        running.current = false
        setImporting(false)
      })
  }

  const restart = () => {
    setLoaded(null)
    setResult(null)
    setError(null)
  }

  const step = result ? 3 : loaded ? 2 : 1
  return (
    <div className="imp">
      <div>
        <Link to={back} go={go} className="muted">
          {t('import.back')}
        </Link>
        <h1>{t('import.title')}</h1>
        <p className="muted">{t('import.lead')}</p>
      </div>
      <ol className="steps" aria-label={t('import.steps')}>
        {STEPS.map((key, i) => (
          <li key={key} className={step === i + 1 ? 'on' : ''} aria-current={step === i + 1 ? 'step' : undefined}>
            {t(key)}
          </li>
        ))}
      </ol>
      <ErrorNote message={error} />
      {step === 1 && <ChooseFile reading={reading} onFile={choose} />}
      {step === 2 && loaded && <Check loaded={loaded} project={proj.name} busy={importing} onImport={start} onBack={restart} />}
      {result && (
        <section className="box">
          <h2>{t('import.done')}</h2>
          <div className="bar" aria-hidden="true">
            <i />
          </div>
          <p className="muted" role="status">
            {t('import.doneBody', {
              name: result.name,
              cards: result.counts.cards,
              lists: result.counts.lists,
              archived: result.counts.archived ? t('import.doneArchived', { n: result.counts.archived }) : '',
            })}
          </p>
          <div className="row">
            <button type="button" className="btn primary" onClick={() => go(boardRoute(company.slug, result.id))}>
              {t('import.open')}
            </button>
            <button type="button" className="btn" onClick={restart}>
              {t('import.again')}
            </button>
          </div>
        </section>
      )}
    </div>
  )
}

function ChooseFile({ reading, onFile }: { reading: boolean; onFile: (file: File | undefined) => void }) {
  const [over, setOver] = useState(false)
  const drag = (e: DragEvent<HTMLDivElement>, on: boolean) => {
    e.preventDefault()
    setOver(on)
  }

  return (
    <>
      <div
        className={`drop${over ? ' over' : ''}`}
        onDragOver={(e) => drag(e, true)}
        onDragLeave={(e) => drag(e, false)}
        onDrop={(e) => {
          drag(e, false)
          onFile(e.dataTransfer.files[0])
        }}
      >
        <b>{t('import.drop')}</b>
        <span className="muted">{t('import.or')}</span>
        <label className={`btn primary${reading ? ' busy' : ''}`}>
          {reading ? t('import.reading') : t('import.choose')}
          <input
            type="file"
            accept=".json,application/json"
            hidden
            disabled={reading}
            onChange={(e) => {
              onFile(e.target.files?.[0])
              e.target.value = ''
            }}
          />
        </label>
        <small className="muted">{t('import.limit')}</small>
      </div>
      <p className="muted where">{t('import.where')}</p>
    </>
  )
}

function Check({ loaded, project, busy, onImport, onBack }: { loaded: Loaded; project: string; busy: boolean; onImport: (people: PeopleChoices, options: ImportChoices) => void; onBack: () => void }) {
  const { preview, file } = loaded
  const [choices, setChoices] = useState<Record<string, string>>(() => Object.fromEntries(preview.people.map((p) => [p.key, suggested(p)])))
  const [options, setOptions] = useState<ImportChoices>({ archived: true, comments: true, links: true })
  const c = preview.counts
  const tiles = (
    [
      [c.lists, 'import.count.lists', true],
      [c.cards, 'import.count.cards', true],
      [c.archived, 'import.count.archived', false],
      [c.labels, 'import.count.labels', false],
      [c.checklists, 'import.count.checklists', false],
      [c.items, 'import.count.items', false],
      [c.comments, 'import.count.comments', false],
      [c.links + c.files, 'import.count.attachments', false],
    ] as [number, MessageKey, boolean][]
  ).filter(([n, , always]) => always || n > 0)

  const submit = () => {
    const changed: PeopleChoices = {}
    for (const p of preview.people) {
      const chosen = choices[p.key] ?? ''
      if (chosen !== suggested(p)) changed[p.key] = chosen ? Number(chosen) : null
    }

    onImport(changed, options)
  }

  const option = (key: keyof ImportChoices, label: MessageKey, hint: MessageKey) => (
    <label className="iopt">
      <input type="checkbox" checked={options[key]} onChange={(e) => setOptions((o) => ({ ...o, [key]: e.target.checked }))} />
      <span>
        <b>{t(label)}</b>
        <small>{t(hint)}</small>
      </span>
    </label>
  )

  return (
    <>
      <section className="box">
        <h2>
          {preview.name} <small className="muted">{t('import.from', { file: file.name })}</small>
        </h2>
        <div className="kv">
          {tiles.map(([n, key]) => (
            <div key={key}>
              <b>{n}</b>
              <small>{t(key)}</small>
            </div>
          ))}
        </div>
        {preview.lists.length > 0 && <p className="muted lists">{t('import.lists', { lists: preview.lists.join(' · ') })}</p>}
      </section>
      {preview.people.length > 0 && (
        <section className="box">
          <h2>{t('import.people')}</h2>
          <p className="muted">{t('import.peopleLead')}</p>
          <table>
            <thead>
              <tr>
                <th>{t('import.inFile')}</th>
                <th>{t('import.inTazko')}</th>
              </tr>
            </thead>
            <tbody>
              {preview.people.map((p) => {
                const name = p.name || p.email
                return (
                  <tr key={p.key}>
                    <td>
                      <b>{name}</b>
                      <br />
                      <small className="muted">{p.email || t('import.noEmail')}</small>
                    </td>
                    <td>
                      <select aria-label={t('import.matchLabel', { name })} value={choices[p.key] ?? ''} onChange={(e) => setChoices((m) => ({ ...m, [p.key]: e.target.value }))}>
                        {preview.candidates.map((x) => (
                          <option key={x.id} value={String(x.id)}>
                            {x.name || x.email}
                          </option>
                        ))}
                        <option value="">{t('import.leaveOut')}</option>
                      </select>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </section>
      )}
      <section className="box">
        <h2>{t('import.options')}</h2>
        {option('archived', 'import.optArchived', 'import.optArchivedHint')}
        {option('comments', 'import.optComments', 'import.optCommentsHint')}
        {option('links', 'import.optLinks', 'import.optLinksHint')}
        {preview.colours > TAZKO_COLOURS && <div className="warnnote">{t('import.colours', { n: preview.colours })}</div>}
      </section>
      <div className="row">
        <button type="button" className="btn primary" disabled={busy} aria-busy={busy} onClick={submit}>
          {busy ? t('import.importing') : t('import.go', { project: preview.project || project })}
        </button>
        <button type="button" className="btn" disabled={busy} onClick={onBack}>
          {t('import.another')}
        </button>
      </div>
    </>
  )
}
