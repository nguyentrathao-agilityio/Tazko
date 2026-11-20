import { useMemo, useState, type KeyboardEvent } from 'react'
import { api } from '@/http'
import { t, type MessageKey } from '@/i18n'
import { ENTER } from '@/keys/keyNames'
import { anchorOf, type Anchor } from '@/ui/anchor'
import { Icon } from '@/ui/Icon'
import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import { Popover } from '@/ui/Popover'
import { useToast } from '@/ui/toast'
import type { BoardData, CardData, ListData } from '@/board/types'
import { dueState, labelStyle } from '@/card/look'
import { LabelsPop, MembersPop } from '@/card/Pops'
import { fieldText, type FieldDef, type FieldValue } from '@/card/fieldTypes'
import { FieldControl } from '@/card/Fields'
import { fromInputs, toDateInput, toTimeInput } from '@/card/dates'
import { appendAfter } from '@/board/order'
import { columns, csv, nextSort, rows as toRows, shownByDefault, sortRows, type Column, type Row, type Sort } from '@/table/table'

type Run = (fn: () => Promise<unknown>) => Promise<boolean>
type Pop = { kind: 'labels' | 'members'; card: CardData; at: Anchor } | { kind: 'columns'; at: Anchor } | null

const STORE = 'tazko.table.'

function loadShown(board: number): Record<string, boolean> {
  try {
    return JSON.parse(window.sessionStorage.getItem(`${STORE}${board}`) ?? '{}') as Record<string, boolean>
  } catch {
    return {}
  }
}

function saveShown(board: number, v: Record<string, boolean>) {
  try {
    window.sessionStorage.setItem(`${STORE}${board}`, JSON.stringify(v))
  } catch {
    return
  }
}

export function TableView({
  board,
  lists,
  total,
  base,
  me,
  run,
  onOpen,
}: {
  board: BoardData
  lists: ListData[]
  total: number
  base: string
  me: number
  run: Run
  onOpen: (card: CardData) => void
}) {
  const toast = useToast()
  const edit = board.can.edit
  const names: Record<string, string> = {
    title: t('table.card'),
    list: t('table.list'),
    labels: t('card.labels'),
    members: t('card.members'),
    start: t('table.start'),
    due: t('table.due'),
    check: t('card.checklist'),
    att: t('table.files'),
    cm: t('table.comments'),
    last: t('table.last'),
  }
  const all = columns(board, names)
  const [shown, setShown] = useState<Record<string, boolean>>(() => loadShown(board.id))
  const [sort, setSort] = useState<Sort>({ key: null, dir: 1 })
  const [group, setGroup] = useState(false)
  const [selected, setSelected] = useState<number[]>([])
  const [pop, setPop] = useState<Pop>(null)
  const listOptions = board.lists.map((l) => (
    <option key={l.id} value={l.id}>
      {l.title}
    </option>
  ))
  const fresh = (card: CardData) => board.lists.flatMap((l) => l.cards).find((c) => c.id === card.id) ?? card
  const isShown = (c: Column) => c.key === 'title' || (shown[c.key] ?? shownByDefault(c))
  const cols = all.filter(isShown)
  const sorted = useMemo(() => sortRows(toRows(lists), sort), [lists, sort])
  const ids = sorted.map((r) => r.card.id)
  const chosen = useMemo(() => new Set(selected), [selected])
  const picked = ids.filter((id) => chosen.has(id))
  const allSelected = ids.length > 0 && picked.length === ids.length

  const patch = (card: CardData, body: Record<string, unknown>) => run(() => api('PATCH', `${base}/cards/${card.id}`, body))
  const move = (card: CardData, list: number) => run(() => api('POST', `${base}/cards/${card.id}/move`, { list_id: list, after: appendAfter(board.lists, list, card.id) }))
  const setField = (card: CardData, f: FieldDef, value: FieldValue | null) => run(() => api('PUT', `${base}/cards/${card.id}/fields/${f.id}`, { value }))
  const moveAll = async (cards: CardData[], list: number) => {
    const moving = new Set(cards.map((c) => c.id))
    const rest = (board.lists.find((l) => l.id === list)?.cards ?? []).filter((c) => !moving.has(c.id))
    let after: number | null = rest.length ? rest[rest.length - 1].id : null
    for (const c of cards) {
      await api('POST', `${base}/cards/${c.id}/move`, { list_id: list, after })
      after = c.id
    }
  }
  const eachAtOnce = (cards: CardData[], call: (c: CardData) => Promise<unknown>) => Promise.all(cards.map(call))
  const toggle = (id: number) => setSelected((s) => (s.includes(id) ? s.filter((x) => x !== id) : [...s, id]))

  const bulk = async (label: MessageKey, fn: (cards: CardData[]) => Promise<unknown>) => {
    const cards = sorted.filter((r) => chosen.has(r.card.id)).map((r) => r.card)
    const ok = await run(() => fn(cards))
    if (ok) {
      toast(t(label, { n: cards.length }))
      setSelected([])
    }
  }

  const copy = () => {
    const text = csv(sorted, cols, board)
    void navigator.clipboard?.writeText(text).then(
      () => toast(t('table.copied', { n: sorted.length })),
      () => toast(t('table.copyBlocked')),
    )
  }

  const header = (c: Column) => {
    const on = sort.key === c.key
    const onKey = (e: KeyboardEvent) => {
      if (e.key === ENTER || e.key === ' ') {
        e.preventDefault()
        setSort(nextSort(sort, c.key))
      }
    }
    return (
      <th key={c.key} tabIndex={0} aria-sort={on ? (sort.dir === 1 ? 'ascending' : 'descending') : 'none'} onClick={() => setSort(nextSort(sort, c.key))} onKeyDown={onKey}>
        {c.key === 'title' ? (
          <span className="row">
            <input
              type="checkbox"
              aria-label={t('table.selectAll')}
              checked={allSelected}
              onClick={(e) => e.stopPropagation()}
              onKeyDown={(e) => e.stopPropagation()}
              onChange={(e) => setSelected(e.target.checked ? ids : [])}
            />
            {c.name}
          </span>
        ) : (
          c.name
        )}
        {on && <i className="sa">{sort.dir === 1 ? ' ▲' : ' ▼'}</i>}
      </th>
    )
  }

  const cell = (r: Row, c: Column) => {
    const card = r.card
    switch (c.key) {
      case 'title':
        return (
          <td key="title" className="tt">
            <div className="row">
              <input type="checkbox" aria-label={t('table.select', { title: card.title })} checked={chosen.has(card.id)} onChange={() => toggle(card.id)} />
              <button type="button" className="chk" role="checkbox" aria-checked={Boolean(card.done)} aria-label={t('card.markComplete')} disabled={!edit} onClick={() => void patch(card, { done: !card.done })}>
                {card.done && <Icon name="check" />}
              </button>
              <button type="button" className={`tl${card.done ? ' dn' : ''}`} title={card.title} onClick={() => onOpen(card)}>
                {card.title}
              </button>
            </div>
          </td>
        )
      case 'list':
        return (
          <td key="list">
            {edit ? (
              <select aria-label={t('table.listOf', { title: card.title })} value={r.list.id} onChange={(e) => void move(card, Number(e.target.value))}>
                {listOptions}
              </select>
            ) : (
              r.list.title
            )}
          </td>
        )
      case 'labels':
        return (
          <td key="labels">
            <div className="lbs" style={{ margin: 0, flexWrap: 'nowrap' }}>
              {(card.labels ?? []).map((id) => {
                const l = board.labels.find((x) => x.id === id)
                return l ? (
                  <span key={id} className="lb" style={labelStyle(l.color)}>
                    {l.name}
                  </span>
                ) : null
              })}
              {edit && (
                <button type="button" className="addb" aria-label={t('table.editLabels', { title: card.title })} onClick={(e) => setPop({ kind: 'labels', card, at: anchorOf(e.currentTarget) })}>
                  <Icon name="plus" />
                </button>
              )}
            </div>
          </td>
        )
      case 'members':
        return (
          <td key="members">
            <span className="row" style={{ flexWrap: 'nowrap', gap: 2 }}>
              {(card.members ?? []).map((id) => {
                const p = board.people.find((x) => x.id === id)
                const n = p ? displayName(p.name, p.email) : '?'
                return (
                  <span key={id} title={n}>
                    <Avatar name={n} />
                  </span>
                )
              })}
              {edit && (
                <button type="button" className="addb" aria-label={t('table.editMembers', { title: card.title })} onClick={(e) => setPop({ kind: 'members', card, at: anchorOf(e.currentTarget) })}>
                  <Icon name="plus" />
                </button>
              )}
            </span>
          </td>
        )
      case 'start':
      case 'due': {
        const k = c.key === 'start' ? 'start_at' : 'due_at'
        const st = c.key === 'due' ? dueState(card.due_at, card.done) : ''
        return (
          <td key={c.key}>
            {edit ? (
              <input
                key={`${card.id}:${card[k] ?? ''}`}
                type="date"
                aria-label={t(c.key === 'due' ? 'table.dueOf' : 'table.startOf', { title: card.title })}
                className={st ? `d-${st}` : undefined}
                defaultValue={toDateInput(card[k])}
                onBlur={(e) => {
                  const next = fromInputs(e.target.value, toTimeInput(card[k]))
                  if (e.target.value !== toDateInput(card[k]) && (next === null || next > 0)) void patch(card, { [k]: next })
                }}
                onKeyDown={(e) => {
                  if (e.key === ENTER) e.currentTarget.blur()
                }}
              />
            ) : (
              toDateInput(card[k])
            )}
          </td>
        )
      }
      case 'check':
        return (
          <td key="check">
            {card.items ? (
              <div className="row">
                <div className="bar2" style={{ width: 64, flex: 'none' }}>
                  <i className={card.items_done === card.items ? 'ok' : undefined} style={{ width: `${Math.round(((card.items_done ?? 0) / card.items) * 100)}%` }} />
                </div>
                <small className="muted">
                  {card.items_done ?? 0}/{card.items}
                </small>
              </div>
            ) : (
              <span className="muted">—</span>
            )}
          </td>
        )
      case 'att':
        return (
          <td key="att" className="num">
            {card.attachments || '—'}
          </td>
        )
      case 'cm':
        return (
          <td key="cm" className="num">
            {card.comments || '—'}
          </td>
        )
      case 'last':
        return <td key="last">{toDateInput(card.updated_at)}</td>
      default: {
        const f = c.field
        if (!f) return <td key={c.key} />
        const v = card.fields?.[String(f.id)]
        return (
          <td key={c.key}>
            {edit ? <FieldControl def={f} value={v} edit label={`${f.name}: ${card.title}`} onSet={(value) => void setField(card, f, value)} /> : fieldText(f, v)}
          </td>
        )
      }
    }
  }

  const line = (r: Row) => (
    <tr key={r.card.id} className={chosen.has(r.card.id) ? 'sel' : undefined}>
      {cols.map((c) => cell(r, c))}
    </tr>
  )

  return (
    <div className="tbd">
      <div className="tbbar">
        {picked.length ? (
          <>
            <b>{t('table.selected', { n: picked.length })}</b>
            {edit && (
              <>
                <select aria-label={t('table.moveSelected')} value="" onChange={(e) => e.target.value && void bulk('table.moved', (cards) => moveAll(cards, Number(e.target.value)))}>
                  <option value="">{t('table.moveTo')}</option>
                  {listOptions}
                </select>
                <button type="button" className="btn sm" onClick={() => void bulk('table.assigned', (cards) => eachAtOnce(cards, (c) => api('PUT', `${base}/cards/${c.id}/members/${me}`, { on: true })))}>
                  {t('table.assignMe')}
                </button>
                <button type="button" className="btn sm" onClick={() => void bulk('table.archived', (cards) => eachAtOnce(cards, (c) => api('POST', `${base}/cards/${c.id}/archive`)))}>
                  {t('card.archive')}
                </button>
              </>
            )}
            <button type="button" className="btn sm quiet" onClick={() => setSelected([])}>
              {t('table.clearSelection')}
            </button>
          </>
        ) : (
          <span className="muted">{t('table.count', { n: sorted.length, total })}</span>
        )}
        <span style={{ flex: 1 }} />
        <button type="button" className="btn sm" aria-pressed={group} onClick={() => setGroup((g) => !g)}>
          <Icon name="list" />
          {group ? t('table.ungroup') : t('table.group')}
        </button>
        <button type="button" className="btn sm" onClick={(e) => setPop({ kind: 'columns', at: anchorOf(e.currentTarget) })}>
          <Icon name="sliders" />
          {t('table.columns')}
        </button>
        <button type="button" className="btn sm" onClick={copy}>
          <Icon name="copy" />
          {t('table.csv')}
        </button>
      </div>
      <div className="tbw">
        <table className="tb">
          <thead>
            <tr>{cols.map(header)}</tr>
          </thead>
          <tbody>
            {sorted.length === 0 && (
              <tr>
                <td colSpan={cols.length} className="muted" style={{ padding: 24, textAlign: 'center' }}>
                  {t('table.none')}
                </td>
              </tr>
            )}
            {group
              ? lists.map((l) => {
                  const rr = sorted.filter((r) => r.list.id === l.id)
                  return [
                    <tr key={`g${l.id}`} className="gh">
                      <td colSpan={cols.length}>
                        {l.title} <span className="muted">· {rr.length}</span>
                      </td>
                    </tr>,
                    ...rr.map(line),
                  ]
                })
              : sorted.map(line)}
            {edit && <AddRow board={board} span={cols.length} base={base} run={run} />}
          </tbody>
        </table>
      </div>
      {pop?.kind === 'columns' && (
        <Popover anchor={pop.at} title={t('table.columns')} width={280} onClose={() => setPop(null)}>
          {all.map((c) => (
            <label key={c.key} className="opt">
              <input
                type="checkbox"
                checked={isShown(c)}
                disabled={c.key === 'title'}
                onChange={(e) => {
                  const next = { ...shown, [c.key]: e.target.checked }
                  setShown(next)
                  saveShown(board.id, next)
                }}
              />
              {c.name}
              {c.key === 'title' && <small className="muted"> {t('table.always')}</small>}
            </label>
          ))}
          <p className="hint" style={{ marginTop: 8 }}>
            {t('table.columnsHint')}
          </p>
        </Popover>
      )}
      {pop?.kind === 'labels' && (
        <Popover anchor={pop.at} title={t('card.labels')} onClose={() => setPop(null)}>
          <LabelsPop
            labels={board.labels}
            chosen={fresh(pop.card).labels ?? []}
            edit={edit}
            onToggle={(id, on) => void run(() => api('PUT', `${base}/cards/${pop.card.id}/labels/${id}`, { on }))}
            onSave={(l) => void run(() => (l.id === null ? api('POST', `${base}/labels`, { name: l.name, color: l.color }) : api('PATCH', `${base}/labels/${l.id}`, { name: l.name, color: l.color })))}
            onDelete={(id) => void run(() => api('DELETE', `${base}/labels/${id}`))}
          />
        </Popover>
      )}
      {pop?.kind === 'members' && (
        <Popover anchor={pop.at} title={t('card.members')} onClose={() => setPop(null)}>
          <MembersPop people={board.people} chosen={fresh(pop.card).members ?? []} onToggle={(id, on) => void run(() => api('PUT', `${base}/cards/${pop.card.id}/members/${id}`, { on }))} />
        </Popover>
      )}
    </div>
  )
}

function AddRow({ board, span, base, run }: { board: BoardData; span: number; base: string; run: Run }) {
  const [title, setTitle] = useState('')
  const [addTo, setAddTo] = useState<number | null>(null)
  const list = addTo ?? board.lists[0]?.id
  const add = () => {
    const v = title.trim()
    if (!v || !list) return
    void run(() => api('POST', `${base}/cards`, { list_id: list, title: v })).then((ok) => {
      if (ok) setTitle('')
    })
  }

  return (
    <tr className="addrow">
      <td colSpan={span}>
        <div className="row">
          <input type="text" placeholder={t('table.addPlaceholder')} aria-label={t('card.title')} style={{ maxWidth: 340 }} value={title} onChange={(e) => setTitle(e.target.value)} onKeyDown={(e) => e.key === ENTER && add()} />
          <select aria-label={t('table.list')} value={list ?? ''} onChange={(e) => setAddTo(Number(e.target.value))}>
            {board.lists.map((l) => (
              <option key={l.id} value={l.id}>
                {l.title}
              </option>
            ))}
          </select>
          <button type="button" className="btn sm primary" onClick={add}>
            {t('common.add')}
          </button>
        </div>
      </td>
    </tr>
  )
}
