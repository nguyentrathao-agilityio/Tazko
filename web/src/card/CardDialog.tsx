import { useCallback, useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from 'react'
import { api } from '@/http'
import { t } from '@/i18n'
import { appendAfter } from '@/board/order'
import { anchorOf, type Anchor } from '@/ui/anchor'
import { Icon } from '@/ui/Icon'
import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import { Popover } from '@/ui/Popover'
import { useToast } from '@/ui/toast'
import { useLoad } from '@/ui/useLoad'
import { useRun } from '@/projects/useRun'
import type { BoardData } from '@/board/types'
import { describe } from '@/card/activity'
import { Editor, type DescriptionSave } from '@/card/Editor'
import { ago, dateRange, dueState, labelStyle } from '@/card/look'
import { AttachPop, AttachmentsSection } from '@/card/Attachments'
import { fileUrl } from '@/card/files'
import { CopyPop, CoverPop, DatesPop, LabelsPop, MembersPop, MovePop, TitlePop } from '@/card/Pops'
import { FieldsSection } from '@/card/Fields'
import { MentionBox } from '@/card/MentionBox'
import { Reactions } from '@/card/Reactions'
import { CommentText } from '@/card/Rich'
import type { CardDetail, Checklist, Comment } from '@/card/types'

type PopKind = 'menu' | 'members' | 'labels' | 'dates' | 'cover' | 'move' | 'copy' | 'checklist' | 'attach'
type Pop = { kind: PopKind; at: Anchor } | null

const ITEMS_SHOWN = 6
const COMMENTS_SHOWN = 4

export function CardDialog({
  me,
  board,
  base,
  cardId,
  link,
  tick = 0,
  onClose,
  onChanged,
  onOpenCard,
}: {
  me: number
  board: BoardData
  base: string
  cardId: number
  link: string
  tick?: number
  onClose: () => void
  onChanged: () => void
  onOpenCard?: (id: number) => void
}) {
  const path = `${base}/cards/${cardId}`
  const { data: card, error: loadError, reload } = useLoad(() => api<CardDetail>('GET', path), path)
  const changed = useCallback(() => {
    reload()
    onChanged()
  }, [reload, onChanged])
  const { error, run } = useRun(changed)
  const toast = useToast()
  const [pop, setPop] = useState<Pop>(null)
  const [editingTitle, setEditingTitle] = useState(false)
  const [editingDesc, setEditingDesc] = useState(false)
  const sheet = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (tick) reload()
  }, [tick, reload])

  useEffect(() => {
    const before = document.activeElement as HTMLElement | null
    sheet.current?.focus()
    return () => before?.focus?.()
  }, [])

  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Escape' && !pop && !editingDesc && !editingTitle) {
      e.stopPropagation()
      onClose()
    }
  }

  const open = (kind: PopKind) => (e: { currentTarget: Element }) => setPop({ kind, at: anchorOf(e.currentTarget) })
  const close = () => setPop(null)
  const patch = (body: Record<string, unknown>) => run(() => api('PATCH', path, body))

  if (loadError)
    return (
      <Shell onClose={onClose} onKey={onKey} sheet={sheet}>
        <div className="cm nocv" style={{ height: 'auto' }}>
          <button type="button" className="ibtn x" aria-label={t('card.close')} onClick={onClose}>
            <Icon name="x" />
          </button>
          <div className="cm-l">
            <h2>{t('card.missing')}</h2>
          </div>
        </div>
      </Shell>
    )

  if (!card)
    return (
      <Shell onClose={onClose} onKey={onKey} sheet={sheet}>
        <p role="status" className="muted" style={{ color: '#fff', textAlign: 'center' }}>
          {t('common.loading')}
        </p>
      </Shell>
    )

  const edit = card.can.edit && !card.archived
  const people = board.people
  const labels = board.labels.filter((l) => card.labels.includes(l.id))
  const members = people.filter((p) => card.members.includes(p.id))
  const state = dueState(card.due_at, card.done)
  const coverImage = card.cover_image ? fileUrl(base, card.cover_image) : null
  const hasCover = Boolean(card.cover || coverImage)

  return (
    <Shell onClose={onClose} onKey={onKey} sheet={sheet}>
      <div className={`cm${hasCover ? '' : ' nocv'}`}>
        <button type="button" className="ibtn x" aria-label={t('card.close')} onClick={onClose}>
          <Icon name="x" />
        </button>
        <button type="button" className="cmenu-btn" aria-haspopup="dialog" title={t('card.menuHint')} onClick={open('menu')}>
          <Icon name="more" />
          {t('card.menu')}
        </button>
        {hasCover && (
          <div className={`cover${coverImage ? ' img' : ''}`} style={coverImage ? undefined : { background: card.cover ?? undefined }}>
            {coverImage && <img src={coverImage} alt="" />}
            {edit && (
              <button type="button" className="btn" onClick={open('cover')}>
                <Icon name="image" />
                {t('card.cover')}
              </button>
            )}
          </div>
        )}
        <div className="cm-g">
          <div className="cm-l">
            {error && (
              <p className="error" role="alert">
                {error}
              </p>
            )}
            <div className="cm-t">
              <button
                type="button"
                className="chk"
                role="checkbox"
                aria-checked={card.done}
                aria-label={t('card.markComplete')}
                disabled={!edit}
                onClick={() => void patch({ done: !card.done })}
              >
                {card.done && <Icon name="check" />}
              </button>
              <h2>
                {editingTitle ? (
                  <TitleEditor
                    initial={card.title}
                    onDone={(v) => {
                      setEditingTitle(false)
                      if (v && v !== card.title) void patch({ title: v })
                    }}
                  />
                ) : (
                  <button type="button" onClick={() => edit && setEditingTitle(true)} aria-label={t('card.renameLabel', { title: card.title })}>
                    {card.title}
                  </button>
                )}
              </h2>
            </div>
            <p className="inl">
              {t('card.inList')}{' '}
              <button type="button" className="lnk" onClick={edit ? open('move') : undefined} disabled={!edit}>
                {card.list.title}
              </button>
              {card.archived && (
                <>
                  {' '}
                  · <b style={{ color: 'var(--warn)' }}>{t('card.archivedNote')}</b>
                </>
              )}
            </p>
            <div className="mrow">
              <div>
                <h4>{t('card.members')}</h4>
                <div className="ln">
                  {members.map((m) => (
                    <span key={m.id} title={displayName(m.name, m.email)}>
                      <Avatar name={displayName(m.name, m.email)} />
                    </span>
                  ))}
                  {edit && (
                    <button type="button" className="addb" aria-label={t('card.addMembers')} onClick={open('members')}>
                      <Icon name="plus" />
                    </button>
                  )}
                </div>
              </div>
              <div>
                <h4>{t('card.labels')}</h4>
                <div className="ln">
                  {labels.map((l) => (
                    <span key={l.id} className="lb" style={labelStyle(l.color)}>
                      {l.name}
                    </span>
                  ))}
                  {edit && (
                    <button type="button" className="addb" aria-label={t('card.editLabels')} onClick={open('labels')}>
                      <Icon name="plus" />
                    </button>
                  )}
                </div>
              </div>
              {(card.due_at || card.start_at) && (
                <div>
                  <h4>{t('card.dates')}</h4>
                  <div className="ln">
                    <button
                      type="button"
                      className="chk"
                      role="checkbox"
                      aria-checked={card.done}
                      aria-label={t('card.markComplete')}
                      disabled={!edit}
                      onClick={() => void patch({ done: !card.done })}
                    >
                      {card.done && <Icon name="check" />}
                    </button>
                    <button type="button" className="datebtn" onClick={edit ? open('dates') : undefined} disabled={!edit}>
                      {dateRange(card.start_at, card.due_at, true)}{' '}
                      {card.done ? (
                        <span className="pill ok">{t('card.complete')}</span>
                      ) : state === 'bad' ? (
                        <span className="pill bad">{t('card.overdue')}</span>
                      ) : state === 'warn' ? (
                        <span className="pill warn">{t('card.dueSoon')}</span>
                      ) : null}
                    </button>
                  </div>
                </div>
              )}
            </div>
            <section className="sec">
              <h3>
                <Icon name="text" />
                {t('card.description')}
                <span className="sp" />
                {edit && card.description && !editingDesc && (
                  <button type="button" className="btn sm" onClick={() => setEditingDesc(true)}>
                    {t('card.edit')}
                  </button>
                )}
              </h3>
              {editingDesc ? (
                <Editor
                  initial={card.description}
                  onCancel={() => setEditingDesc(false)}
                  onSave={(body: DescriptionSave) =>
                    void patch(body).then((ok) => {
                      if (ok) setEditingDesc(false)
                    })
                  }
                />
              ) : card.description ? (
                <div className="desc" dangerouslySetInnerHTML={{ __html: card.description }} onDoubleClick={() => edit && setEditingDesc(true)} />
              ) : edit ? (
                <button type="button" className="desc empty" onClick={() => setEditingDesc(true)}>
                  {t('card.addDescription')}
                </button>
              ) : (
                <div className="desc empty">{t('boardMenu.noDescription')}</div>
              )}
            </section>
            <FieldsSection defs={board.fields ?? []} values={card.fields ?? {}} edit={edit} path={path} run={run} />
            {card.checklists.map((k, i) => (
              <ChecklistBlock
                key={k.id}
                checklist={k}
                index={i}
                all={card.checklists}
                edit={edit}
                base={base}
                run={run}
              />
            ))}
            {edit && (
              <div className="ckadd">
                <button type="button" className="btn sm" onClick={open('checklist')}>
                  <Icon name="plus" />
                  {t('card.addChecklist')}
                </button>
              </div>
            )}
            <AttachmentsSection card={card} base={base} edit={edit} run={run} onAdd={(el) => setPop({ kind: 'attach', at: anchorOf(el) })} />
          </div>
          <aside className="cm-cm" aria-label={t('card.commentsAndActivity')}>
            <Feed me={me} card={card} board={board} edit={edit} base={base} path={path} run={run} />
          </aside>
        </div>
      </div>
      {pop && (
        <Popover
          anchor={pop.at}
          title={popTitle(pop.kind)}
          width={pop.kind === 'menu' ? 270 : 320}
          onClose={close}
        >
          {pop.kind === 'menu' && (
            <CardMenu
              card={card}
              edit={edit}
              me={me}
              people={board.people}
              onPick={(kind) => setPop({ kind, at: pop.at })}
              onJoin={(id) => void run(() => api('PUT', `${path}/members/${id}`, { on: true })).then(close)}
              onArchive={() =>
                void run(() => api('POST', `${path}/${card.archived ? 'restore' : 'archive'}`)).then((ok) => {
                  close()
                  if (ok) toast(card.archived ? t('card.restored') : t('card.archived'))
                })
              }
              hasFields={Boolean(board.fields?.length)}
              onTemplate={() =>
                void run(() => api('PUT', `${path}/template`, { on: !card.is_template })).then((ok) => {
                  close()
                  if (ok) toast(card.is_template ? t('ctpl.stopped') : t('ctpl.made'))
                })
              }
              onFields={() => {
                close()
                document.getElementById('card-fields')?.scrollIntoView({ behavior: 'smooth', block: 'center' })
              }}
              onShare={() => {
                const url = `${window.location.origin}${link}`
                void navigator.clipboard?.writeText(url).then(
                  () => toast(t('card.linkCopied')),
                  () => toast(url),
                )
                close()
              }}
            />
          )}
          {pop.kind === 'members' && (
            <MembersPop people={people} chosen={card.members} onToggle={(id, on) => void run(() => api('PUT', `${path}/members/${id}`, { on }))} />
          )}
          {pop.kind === 'labels' && (
            <LabelsPop
              labels={board.labels}
              chosen={card.labels}
              edit={edit}
              onToggle={(id, on) => void run(() => api('PUT', `${path}/labels/${id}`, { on }))}
              onSave={(l) =>
                void run(() =>
                  l.id === null ? api('POST', `${base}/labels`, { name: l.name, color: l.color }) : api('PATCH', `${base}/labels/${l.id}`, { name: l.name, color: l.color }),
                )
              }
              onDelete={(id) => void run(() => api('DELETE', `${base}/labels/${id}`))}
            />
          )}
          {pop.kind === 'dates' && (
            <DatesPop
              start={card.start_at}
              due={card.due_at}
              onSave={(v) =>
                void patch(v).then((ok) => {
                  if (ok) close()
                })
              }
            />
          )}
          {pop.kind === 'cover' && (
            <CoverPop
              cover={card.cover}
              coverImage={card.cover_image}
              images={card.attachments.filter((a) => a.image).map((a) => ({ id: a.id, name: a.name, src: fileUrl(base, a.id) }))}
              onPick={(c) =>
                void patch({ cover: c }).then((ok) => {
                  if (ok) close()
                })
              }
              onPickImage={(id) =>
                void run(() => api('PUT', `${base}/attachments/${id}/cover`)).then((ok) => {
                  if (ok) close()
                })
              }
            />
          )}
          {pop.kind === 'move' && (
            <MovePop
              lists={board.lists}
              current={card.list.id}
              label={t('list.moveButton')}
              onMove={(list, top) => {
                const after = appendAfter(board.lists, list, card.id, top)
                void run(() => api('POST', `${path}/move`, { list_id: list, after })).then((ok) => {
                  if (ok) close()
                })
              }}
            />
          )}
          {pop.kind === 'copy' && (
            <CopyPop
              title={card.title}
              lists={board.lists}
              current={card.list.id}
              onCopy={(title, list) =>
                void api<{ id: number }>('POST', `${path}/copy`, { title, list_id: list }).then((r) => {
                  close()
                  onChanged()
                  toast(t('card.copied'))
                  onOpenCard?.(r.id)
                })
              }
            />
          )}
          {pop.kind === 'attach' && (
            <AttachPop
              base={base}
              path={path}
              onDone={(name) => {
                close()
                changed()
                toast(t('attach.uploaded', { name }))
              }}
            />
          )}
          {pop.kind === 'checklist' && (
            <TitlePop
              initial={t('card.checklistDefault')}
              label={t('common.add')}
              onSave={(title) =>
                void run(() => api('POST', `${path}/checklists`, { title })).then((ok) => {
                  if (ok) close()
                })
              }
            />
          )}
        </Popover>
      )}
    </Shell>
  )
}

function popTitle(kind: PopKind): string {
  const titles: Record<PopKind, string> = {
    menu: t('card.menu'),
    members: t('card.members'),
    labels: t('card.labels'),
    dates: t('card.dates'),
    cover: t('card.cover'),
    move: t('card.moveTitle'),
    copy: t('card.copyTitle'),
    checklist: t('card.addChecklist'),
    attach: t('attach.popTitle'),
  }
  return titles[kind]
}

function Shell({
  children,
  onClose,
  onKey,
  sheet,
}: {
  children: ReactNode
  onClose: () => void
  onKey: (e: KeyboardEvent) => void
  sheet: React.RefObject<HTMLDivElement | null>
}) {
  return (
    <div
      className="ov cmov"
      role="dialog"
      aria-modal="true"
      aria-label={t('card.dialog')}
      tabIndex={-1}
      ref={sheet}
      onKeyDown={onKey}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      {children}
    </div>
  )
}

function TitleEditor({ initial, onDone }: { initial: string; onDone: (v: string) => void }) {
  const [v, setV] = useState(initial)
  return (
    <textarea
      className="ta-s"
      value={v}
      aria-label={t('card.title')}
      autoFocus
      onChange={(e) => setV(e.target.value)}
      onBlur={() => onDone(v.trim())}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          onDone(v.trim())
        }

        if (e.key === 'Escape') {
          e.stopPropagation()
          onDone(initial)
        }
      }}
    />
  )
}

function CardMenu({
  card,
  edit,
  me,
  people,
  onPick,
  onJoin,
  onArchive,
  onShare,
  hasFields,
  onFields,
  onTemplate,
}: {
  card: CardDetail
  edit: boolean
  me: number
  people: BoardData['people']
  onPick: (k: PopKind) => void
  onJoin: (id: number) => void
  onArchive: () => void
  onShare: () => void
  hasFields: boolean
  onFields: () => void
  onTemplate: () => void
}) {
  const self = people.find((p) => p.id === me)
  const item = (icon: Parameters<typeof Icon>[0]['name'], label: string, act: () => void) => (
    <button type="button" className="mi" onClick={act}>
      <Icon name={icon} />
      {label}
    </button>
  )
  return (
    <>
      {edit && (
        <>
          <div className="pg2">{t('card.addToCard')}</div>
          {self && self.role !== 'observer' && !card.members.includes(self.id) && item('user', t('card.join'), () => onJoin(self.id))}
          {item('user', t('card.members'), () => onPick('members'))}
          {item('tag', t('card.labels'), () => onPick('labels'))}
          {item('checksq', t('card.checklist'), () => onPick('checklist'))}
          {item('clock', t('card.dates'), () => onPick('dates'))}
          {item('clip', t('attach.attachment'), () => onPick('attach'))}
          {item('image', t('card.cover'), () => onPick('cover'))}
          {hasFields && item('sliders', t('fields.title'), onFields)}
        </>
      )}
      <div className="pg2">{t('card.actions')}</div>
      {edit && item('move', t('card.move'), () => onPick('move'))}
      {edit && item('copy', t('card.copy'), () => onPick('copy'))}
      {card.can.edit && item(card.archived ? 'undo' : 'archive', card.archived ? t('card.restore') : t('card.archive'), onArchive)}
      {edit && item('tmpl', card.is_template ? t('ctpl.stop') : t('ctpl.make'), onTemplate)}
      {item('share', t('card.shareLink'), onShare)}
    </>
  )
}

function ChecklistBlock({
  checklist,
  index,
  all,
  edit,
  base,
  run,
}: {
  checklist: Checklist
  index: number
  all: Checklist[]
  edit: boolean
  base: string
  run: (fn: () => Promise<unknown>) => Promise<boolean>
}) {
  const [hide, setHide] = useState(false)
  const [expanded, setExpanded] = useState(false)
  const [adding, setAdding] = useState(false)
  const [text, setText] = useState('')
  const [renaming, setRenaming] = useState(false)
  const [editingItem, setEditingItem] = useState<number | null>(null)
  const total = checklist.items.length
  const done = checklist.items.filter((i) => i.done).length
  const pct = total ? Math.round((done / total) * 100) : 0
  const items = checklist.items.filter((i) => !(hide && i.done))
  const shown = expanded ? items : items.slice(0, ITEMS_SHOWN)
  const k = `${base}/checklists/${checklist.id}`

  const add = () => {
    const v = text.trim()
    if (!v) return
    void run(() => api('POST', `${k}/items`, { text: v })).then((ok) => {
      if (ok) setText('')
    })
  }

  const moveItem = (from: number, dir: -1 | 1) => {
    const ids = checklist.items.map((i) => i.id)
    const to = from + dir
    if (to < 0 || to >= ids.length) return
    const without = ids.filter((_, i) => i !== from)
    const after = to === 0 ? null : without[to - 1]
    void run(() => api('POST', `${base}/items/${ids[from]}/move`, { after }))
  }

  return (
    <section className="ckb">
      <div className="ckh">
        <Icon name="checksq" />
        <h3>
          {renaming ? (
            <input
              className="inl-i"
              defaultValue={checklist.title}
              aria-label={t('cardPop.title')}
              autoFocus
              onBlur={(e) => {
                setRenaming(false)
                const v = e.currentTarget.value.trim()
                if (v && v !== checklist.title) void run(() => api('PATCH', k, { title: v }))
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter') e.currentTarget.blur()
                if (e.key === 'Escape') {
                  e.stopPropagation()
                  setRenaming(false)
                }
              }}
            />
          ) : (
            <button type="button" onClick={() => edit && setRenaming(true)}>
              {checklist.title}
            </button>
          )}
        </h3>
        {edit && all.length > 1 && (
          <>
            <button
              type="button"
              className="ibtn sm up"
              aria-label={t('card.checklistUp')}
              disabled={index === 0}
              onClick={() => void run(() => api('POST', `${k}/move`, { after: index <= 1 ? null : all[index - 2].id }))}
            >
              <Icon name="chev" />
            </button>
            <button
              type="button"
              className="ibtn sm"
              aria-label={t('card.checklistDown')}
              disabled={index === all.length - 1}
              onClick={() => void run(() => api('POST', `${k}/move`, { after: all[index + 1].id }))}
            >
              <Icon name="chev" />
            </button>
          </>
        )}
        {done > 0 && (
          <button type="button" className="btn sm" onClick={() => setHide((h) => !h)}>
            {hide ? t('card.showChecked') : t('card.hideChecked', { n: done })}
          </button>
        )}
        {edit && (
          <button type="button" className="btn sm" onClick={() => void run(() => api('DELETE', k))}>
            {t('manage.delete')}
          </button>
        )}
      </div>
      <div className="prog">
        <span>{pct}%</span>
        <div className="bar2">
          <i className={pct === 100 ? 'ok' : ''} style={{ width: `${pct}%` }} />
        </div>
      </div>
      {shown.map((it) => {
        const at = checklist.items.findIndex((x) => x.id === it.id)
        return (
          <div key={it.id} className={`it${it.done ? ' d' : ''}`}>
            <button
              type="button"
              className="cbx"
              role="checkbox"
              aria-checked={it.done}
              aria-label={it.text}
              disabled={!edit}
              onClick={() => void run(() => api('PATCH', `${base}/items/${it.id}`, { done: !it.done }))}
            >
              {it.done && <Icon name="check" />}
            </button>
            {editingItem === it.id ? (
              <input
                className="inl-i"
                defaultValue={it.text}
                aria-label={t('card.itemText')}
                autoFocus
                onBlur={(e) => {
                  setEditingItem(null)
                  const v = e.currentTarget.value.trim()
                  if (v && v !== it.text) void run(() => api('PATCH', `${base}/items/${it.id}`, { text: v }))
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') e.currentTarget.blur()
                  if (e.key === 'Escape') {
                    e.stopPropagation()
                    setEditingItem(null)
                  }
                }}
              />
            ) : (
              <button
                type="button"
                className="itx"
                onClick={() => edit && setEditingItem(it.id)}
                onKeyDown={(e) => {
                  if (!edit || !e.altKey) return
                  if (e.key === 'ArrowUp') {
                    e.preventDefault()
                    moveItem(at, -1)
                  }

                  if (e.key === 'ArrowDown') {
                    e.preventDefault()
                    moveItem(at, 1)
                  }
                }}
                title={edit ? t('card.itemMoveHint') : undefined}
              >
                {it.text}
              </button>
            )}
            {edit && (
              <span className="acts">
                <button type="button" className="ibtn sm" aria-label={t('card.deleteItem')} onClick={() => void run(() => api('DELETE', `${base}/items/${it.id}`))}>
                  <Icon name="trash" />
                </button>
              </span>
            )}
          </div>
        )
      })}
      {items.length > ITEMS_SHOWN && (
        <button type="button" className="more ind" aria-expanded={expanded} onClick={() => setExpanded((e) => !e)}>
          {expanded ? t('card.showFewer') : t('card.showAllItems', { n: items.length })}
        </button>
      )}
      {edit &&
        (adding ? (
          <div style={{ margin: '6px 0 0 28px' }}>
            <input
              type="text"
              className="inl-i"
              placeholder={t('card.addItem')}
              aria-label={t('card.newItem')}
              value={text}
              autoFocus
              onChange={(e) => setText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault()
                  add()
                }

                if (e.key === 'Escape') {
                  e.stopPropagation()
                  setAdding(false)
                }
              }}
            />
            <div className="row" style={{ marginTop: 6 }}>
              <button type="button" className="btn primary sm" onClick={add}>
                {t('common.add')}
              </button>
              <button type="button" className="btn sm" onClick={() => setAdding(false)}>
                {t('common.cancel')}
              </button>
            </div>
          </div>
        ) : (
          <button type="button" className="btn sm" style={{ margin: '6px 0 0 28px' }} onClick={() => setAdding(true)}>
            {t('card.addItem')}
          </button>
        ))}
    </section>
  )
}

type FeedEntry = { at: number; kind: 'comment'; comment: Comment } | { at: number; kind: 'event'; event: CardDetail['activity'][number] }

function Feed({
  me,
  card,
  board,
  edit,
  base,
  path,
  run,
}: {
  me: number
  card: CardDetail
  board: BoardData
  edit: boolean
  base: string
  path: string
  run: (fn: () => Promise<unknown>) => Promise<boolean>
}) {
  const [details, setDetails] = useState(false)
  const [expanded, setExpanded] = useState(false)
  const [draft, setDraft] = useState('')
  const [editing, setEditing] = useState<{ id: number; text: string } | null>(null)

  const feed: FeedEntry[] = card.comments.map((c) => ({ at: c.created_at, kind: 'comment' as const, comment: c }))
  if (details)
    card.activity
      .filter((e) => !e.type.startsWith('comment_'))
      .forEach((e) => feed.push({ at: e.created_at, kind: 'event', event: e }))
  feed.sort((a, b) => b.at - a.at)
  const shown = expanded ? feed : feed.slice(0, COMMENTS_SHOWN)

  const save = () => {
    const v = draft.trim()
    if (!v) return
    void run(() => api('POST', `${path}/comments`, { body: v })).then((ok) => {
      if (ok) setDraft('')
    })
  }

  const ctx = { lists: board.lists, labels: board.labels, people: board.people }
  return (
    <section className="sec">
      <h3>
        <Icon name="msg" />
        {t('card.commentsAndActivity')}
        <span className="sp" />
        <button type="button" className="btn sm" onClick={() => setDetails((d) => !d)}>
          {details ? t('card.hideDetails') : t('card.showDetails')}
        </button>
      </h3>
      {edit && (
        <div className="cmt">
          <Avatar name={displayName(board.people.find((p) => p.id === me)?.name ?? '', board.people.find((p) => p.id === me)?.email ?? '?')} />
          <div className="bub">
            <MentionBox
              value={draft}
              people={board.people.filter((p) => p.access && p.id !== me)}
              placeholder={t('card.commentPlaceholder')}
              label={t('card.comment')}
              onChange={setDraft}
              onSubmit={save}
            />
            <div className="row" style={{ marginTop: 6 }}>
              <button type="button" className="btn primary sm" onClick={save}>
                {t('boardMenu.save')}
              </button>
              <small className="muted">{t('card.ctrlEnter')}</small>
            </div>
          </div>
        </div>
      )}
      {shown.map((e) => {
        if (e.kind === 'event') {
          const who = e.event.actor ? displayName(e.event.actor.name, e.event.actor.email) : t('card.someone')
          return (
            <div key={`e${e.event.id}`} className="actl">
              <Avatar name={who} />
              <div>
                <b style={{ color: 'var(--ink)' }}>{who}</b> {describe(e.event, ctx)} <small>{ago(e.at)}</small>
              </div>
            </div>
          )
        }

        const c = e.comment
        const who = c.author ? displayName(c.author.name, c.author.email) : t('card.someone')
        const mine = c.author?.id === me
        return (
          <div key={`c${c.id}`} className="cmt">
            <Avatar name={who} />
            <div className="bub">
              <div className="who2">
                <b>{who}</b>
                <time>
                  {ago(c.created_at)}
                  {c.edited_at ? ` ${t('card.edited')}` : ''}
                </time>
              </div>
              {editing?.id === c.id ? (
                <>
                  <textarea className="ta-s" value={editing.text} aria-label={t('card.comment')} onChange={(ev) => setEditing({ id: c.id, text: ev.target.value })} autoFocus />
                  <div className="row" style={{ marginTop: 6 }}>
                    <button
                      type="button"
                      className="btn primary sm"
                      onClick={() =>
                        void run(() => api('PATCH', `${base}/comments/${c.id}`, { body: editing.text })).then((ok) => {
                          if (ok) setEditing(null)
                        })
                      }
                    >
                      {t('boardMenu.save')}
                    </button>
                    <button type="button" className="btn sm" onClick={() => setEditing(null)}>
                      {t('common.cancel')}
                    </button>
                  </div>
                </>
              ) : (
                <div className="body">
                  <CommentText text={c.body} names={board.people.flatMap((p) => [p.name, p.email, p.email.split('@')[0]])} />
                </div>
              )}
              <Reactions comment={c.id} reactions={c.reactions ?? []} edit={edit} base={base} run={run} />
              {mine && edit && editing?.id !== c.id && (
                <div className="acts">
                  <button type="button" onClick={() => setEditing({ id: c.id, text: c.body })}>
                    {t('card.edit')}
                  </button>
                  <button type="button" onClick={() => void run(() => api('DELETE', `${base}/comments/${c.id}`))}>
                    {t('manage.delete')}
                  </button>
                </div>
              )}
            </div>
          </div>
        )
      })}
      {feed.length > COMMENTS_SHOWN && (
        <button type="button" className="more" aria-expanded={expanded} onClick={() => setExpanded((x) => !x)}>
          {expanded ? t('card.showFewer') : details ? t('card.showAllFeed', { n: feed.length }) : t('card.showAllComments', { n: feed.length })}
        </button>
      )}
      {feed.length === 0 && !edit && <p className="hint">{t('card.noComments')}</p>}
    </section>
  )
}
