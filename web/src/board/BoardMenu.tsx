import { useEffect, useState } from 'react'
import { api, HttpError, upload } from '@/http'
import { t } from '@/i18n'
import { Icon, type IconName } from '@/ui/Icon'
import { useLoad } from '@/ui/useLoad'
import { background, backgroundNames } from '@/projects/look'
import { Access } from '@/board/Access'
import { describeBoard, type BoardEntry } from '@/card/activity'
import { ago } from '@/card/look'
import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import { FieldsPage } from '@/card/Fields'
import type { BoardData } from '@/board/types'
import { ESCAPE } from '@/keys/keyNames'

type Page = 'main' | 'about' | 'bg' | 'tpl' | 'fields' | 'arch' | 'access' | 'export' | 'copy' | 'activity'

export function BoardMenu({
  board,
  base,
  run,
  onClose,
  onCopied,
  onClosed,
}: {
  board: BoardData
  base: string
  run: (fn: () => Promise<unknown>) => Promise<boolean>
  onClose: () => void
  onCopied: (id: number) => void
  onClosed: () => void
}) {
  const [page, setPage] = useState<Page>('main')

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === ESCAPE) onClose()
    }
    window.addEventListener('keydown', onKey)

    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  const titles: Record<Page, string> = {
    main: t('boardMenu.title'),
    about: t('boardMenu.about'),
    bg: t('boardMenu.background'),
    arch: t('boardMenu.archived'),
    access: t('boardMenu.access'),
    export: t('boardMenu.export'),
    copy: t('boardMenu.copy'),
    activity: t('boardMenu.activity'),
    fields: t('fields.title'),
    tpl: t('tpl.save'),
  }
  const item = (to: Page, icon: IconName, label: string) => (
    <button type="button" className="mi" onClick={() => setPage(to)}>
      <Icon name={icon} />
      {label}
    </button>
  )

  return (
    <aside className="drawer" aria-label={t('boardMenu.title')}>
      <div className="pop-h">
        {page === 'main' ? (
          <span style={{ width: 26 }} />
        ) : (
          <button type="button" className="ibtn sm" onClick={() => setPage('main')} aria-label={t('pop.back')}>
            <Icon name="back" />
          </button>
        )}
        <b>{titles[page]}</b>
        <button type="button" className="ibtn sm" onClick={onClose} aria-label={t('pop.close')}>
          <Icon name="x" />
        </button>
      </div>
      <div className="pop-b">
        {page === 'main' && (
          <>
            {item('about', 'info', t('boardMenu.about'))}
            {board.can.edit && item('bg', 'palette', t('boardMenu.background'))}
            {item('fields', 'sliders', t('fields.title'))}
            {item('arch', 'archive', t('boardMenu.archived'))}
            {item('activity', 'act', t('boardMenu.activity'))}
            <button
              type="button"
              className="mi"
              aria-pressed={board.watching}
              onClick={() => void run(() => api('PUT', `${base}/watch`, { on: !board.watching }))}
            >
              <Icon name="eye" />
              {board.watching ? t('board.unwatch') : t('board.watch')}
            </button>
            {board.can.manage && item('access', 'lock', t('boardMenu.access'))}
            {item('export', 'upload', t('boardMenu.export'))}
            {board.can.create_board && (
              <>
                <hr />
                {item('copy', 'copy', t('boardMenu.copy'))}
                {board.can.manage && item('tpl', 'tmpl', t('tpl.save'))}
              </>
            )}
            {board.can.manage && (
              <button
                type="button"
                className="mi danger"
                onClick={() =>
                  void run(() => api('POST', `${base}/close`)).then((ok) => {
                    if (ok) onClosed()
                  })
                }
              >
                <Icon name="archive" />
                {t('boardMenu.close')}
              </button>
            )}
          </>
        )}
        {page === 'about' && <About board={board} base={base} run={run} />}
        {page === 'fields' && <FieldsPage defs={board.fields ?? []} edit={board.can.edit} base={base} run={run} />}
        {page === 'bg' && <BackgroundPage board={board} base={base} run={run} />}
        {page === 'tpl' && <SaveTemplate board={board} base={base} onDone={() => setPage('main')} />}
        {page === 'arch' && <Archived board={board} base={base} run={run} />}
        {page === 'access' && <Access board={board} base={base} run={run} />}
        {page === 'activity' && <Activity board={board} base={base} />}
        {page === 'export' && (
          <>
            <p>{t('boardMenu.exportBody', { name: board.name })}</p>
            <a className="btn primary sm" href={`${base}/export`} download>
              {t('boardMenu.exportDownload')}
            </a>
            <p className="hint" style={{ marginTop: 10 }}>
              {t('boardMenu.exportHint')}
            </p>
          </>
        )}
        {page === 'copy' && (
          <form
            onSubmit={(e) => {
              e.preventDefault()
              const name = String(new FormData(e.currentTarget).get('name') ?? '').trim()
              if (!name) return
              void api<{ id: number }>('POST', `${base}/copy`, { name }).then((r) => onCopied(r.id))
            }}
          >
            <div className="field">
              <label htmlFor="cb-n">{t('boardMenu.copyName')}</label>
              <input type="text" id="cb-n" name="name" defaultValue={t('boardMenu.copyDefault', { name: board.name })} />
            </div>
            <button type="submit" className="btn primary sm">
              {t('board.create')}
            </button>
          </form>
        )}
      </div>
    </aside>
  )
}

function About({ board, base, run }: { board: BoardData; base: string; run: (fn: () => Promise<unknown>) => Promise<boolean> }) {
  const [text, setText] = useState(board.description)
  if (!board.can.edit) return <p>{board.description || t('boardMenu.noDescription')}</p>
  return (
    <>
      <label htmlFor="ab-t" className="hint" style={{ padding: 0, border: 0, display: 'block' }}>
        {t('project.description')}
      </label>
      <textarea id="ab-t" value={text} onChange={(e) => setText(e.target.value)} />
      <button type="button" className="btn primary sm" style={{ marginTop: 8 }} onClick={() => void run(() => api('PATCH', base, { description: text }))}>
        {t('boardMenu.save')}
      </button>
    </>
  )
}

function Archived({ board, base, run }: { board: BoardData; base: string; run: (fn: () => Promise<unknown>) => Promise<boolean> }) {
  const path = `${base}/archive`
  const { data, reload } = useLoad(
    () => api<{ lists: { id: number; title: string }[]; cards: { id: number; title: string; list: string }[] }>('GET', path),
    `${path}:${board.lists.length}:${board.archived_lists}`,
  )
  return (
    <>
      <h4 style={{ marginTop: 0 }}>{t('boardMenu.archivedCards', { n: data?.cards.length ?? 0 })}</h4>
      {data && data.cards.length === 0 && <p className="hint">{t('boardMenu.nothingArchived')}</p>}
      {data?.cards.map((c) => (
        <div key={c.id} className="row" style={{ flexWrap: 'nowrap', padding: '6px 0', borderBottom: '1px solid var(--line)' }}>
          <span style={{ flex: 1 }}>
            {c.title} <small className="muted">{c.list}</small>
          </span>
          {board.can.edit && (
            <button type="button" className="btn sm" aria-label={t('boardMenu.restoreCard', { title: c.title })} onClick={() => void run(() => api('POST', `${base}/cards/${c.id}/restore`)).then(reload)}>
              {t('manage.restore')}
            </button>
          )}
        </div>
      ))}
      <h4>{t('boardMenu.archivedLists', { n: data?.lists.length ?? 0 })}</h4>
      {data && data.lists.length === 0 && <p className="hint">{t('boardMenu.nothingArchived')}</p>}
      {data?.lists.map((l) => (
        <div key={l.id} className="row" style={{ flexWrap: 'nowrap', padding: '6px 0', borderBottom: '1px solid var(--line)' }}>
          <span style={{ flex: 1 }}>{l.title}</span>
          {board.can.edit && (
            <button type="button" className="btn sm" onClick={() => void run(() => api('POST', `${base}/lists/${l.id}/restore`)).then(reload)}>
              {t('manage.restore')}
            </button>
          )}
        </div>
      ))}
    </>
  )
}

function Activity({ board, base }: { board: BoardData; base: string }) {
  const [older, setOlder] = useState<BoardEntry[]>([])
  const [done, setDone] = useState(false)
  const path = `${base}/activity`
  const { data } = useLoad(() => api<{ entries: BoardEntry[] }>('GET', path), path)
  const entries = [...(data?.entries ?? []), ...older]
  const ctx = { lists: board.lists, labels: board.labels, people: board.people }
  const more = () => {
    const last = entries[entries.length - 1]
    if (!last) return
    void api<{ entries: BoardEntry[] }>('GET', `${path}?before=${last.id}`).then((r) => {
      setOlder((o) => [...o, ...r.entries])
      if (r.entries.length < 50) setDone(true)
    })
  }

  return (
    <>
      {data && entries.length === 0 && <p className="hint">{t('boardMenu.noActivity')}</p>}
      {entries.map((e) => {
        const who = e.actor ? displayName(e.actor.name, e.actor.email) : t('card.someone')
        return (
          <div key={e.id} className="actl">
            <Avatar name={who} />
            <div>
              <b style={{ color: 'var(--ink)' }}>{who}</b> {describeBoard(e, ctx)}
              <br />
              <small>{ago(e.created_at)}</small>
            </div>
          </div>
        )
      })}
      {data && data.entries.length === 50 && !done && (
        <button type="button" className="more" onClick={more}>
          {t('common.loadMore')}
        </button>
      )}
    </>
  )
}

function BackgroundPage({ board, base, run }: { board: BoardData; base: string; run: (fn: () => Promise<unknown>) => Promise<boolean> }) {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const uploaded = board.background.startsWith('img:')
  const pick = (file: File | undefined) => {
    if (!file) return
    if (file.size > 5 * 1024 * 1024) {
      setError(t('bg.tooLarge'))
      return
    }

    setBusy(true)
    setError(null)
    void run(() =>
      upload(`${base}/background`, file).catch((e: unknown) => {
        setError(e instanceof HttpError ? e.message : String(e))
        throw e
      }),
    ).finally(() => setBusy(false))
  }

  return (
    <>
      <div className="swatches">
        {uploaded && (
          <button type="button" className="sw" style={{ background: background(board.background, base), width: 96, height: 62 }} aria-pressed aria-label={t('bg.current')} />
        )}
        {backgroundNames.map((k) => (
          <button
            key={k}
            type="button"
            className="sw"
            style={{ background: background(k), width: 96, height: 62 }}
            aria-pressed={board.background === k}
            aria-label={t('project.backgroundOption', { name: k })}
            disabled={!board.can.edit}
            onClick={() => void run(() => api('PATCH', base, { background: k }))}
          />
        ))}
        {board.can.edit && (
          <label className={`sw up${busy ? ' busy' : ''}`} title={t('bg.upload')}>
            <Icon name="upload" />
            <input type="file" accept="image/png,image/jpeg,image/gif,image/webp" style={{ display: 'none' }} aria-label={t('bg.upload')} disabled={busy} onChange={(e) => pick(e.target.files?.[0])} />
          </label>
        )}
      </div>
      {error && (
        <p className="error" role="alert" style={{ marginTop: 10 }}>
          {error}
        </p>
      )}
      <p className="hint" style={{ marginTop: 10 }}>
        {busy ? t('bg.uploading') : t('bg.hint')}
      </p>
    </>
  )
}

function SaveTemplate({ board, base, onDone }: { board: BoardData; base: string; onDone: () => void }) {
  const [name, setName] = useState(t('tpl.nameDefault', { name: board.name }))
  const [description, setDescription] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [saved, setSaved] = useState<{ cards_left_out: boolean } | null>(null)
  const save = () =>
    void api<{ cards_left_out: boolean }>('POST', `${base}/save-template`, { name, description }).then(
      (r) => {
        setSaved(r)
        setError(null)
      },
      (e: unknown) => setError(e instanceof HttpError ? e.message : String(e)),
    )

  if (saved)
    return (
      <>
        <p role="status">{t(saved.cards_left_out ? 'tpl.savedWithoutCards' : 'tpl.saved')}</p>
        <button type="button" className="btn sm" onClick={onDone}>
          {t('pop.back')}
        </button>
      </>
    )
  return (
    <>
      <div className="field">
        <label htmlFor="st-n">{t('tpl.saveName')}</label>
        <input type="text" id="st-n" value={name} onChange={(e) => setName(e.target.value)} />
      </div>
      <div className="field">
        <label htmlFor="st-d">{t('tpl.saveDesc')}</label>
        <input type="text" id="st-d" placeholder={t('tpl.saveDescHint')} value={description} onChange={(e) => setDescription(e.target.value)} />
      </div>
      <p className="hint">{t(board.restricted ? 'tpl.saveHintRestricted' : 'tpl.saveHint')}</p>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <button type="button" className="btn primary sm" onClick={save}>
        {t('tpl.saveButton')}
      </button>
    </>
  )
}
