import { useState, type FormEvent } from 'react'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import type { BoardPerson, Label, ListData } from '@/board/types'
import { labelColors, labelStyle } from '@/card/look'

function search(q: string, text: string) {
  return !q || text.toLowerCase().includes(q.toLowerCase())
}

export function MembersPop({ people, chosen, onToggle }: { people: BoardPerson[]; chosen: number[]; onToggle: (id: number, on: boolean) => void }) {
  const [q, setQ] = useState('')
  return (
    <>
      <input type="search" value={q} onChange={(e) => setQ(e.target.value)} placeholder={t('cardPop.searchMembers')} aria-label={t('cardPop.searchMembers')} data-focus />
      <div style={{ marginTop: 8 }}>
        {people
          .filter((p) => p.access && search(q, `${p.name} ${p.email}`))
          .map((p) => {
            const name = displayName(p.name, p.email)
            const on = chosen.includes(p.id)
            const observer = p.role === 'observer'
            return (
              <button
                key={p.id}
                type="button"
                className="mr"
                aria-pressed={on}
                disabled={observer}
                title={observer ? t('cardPop.observerNote') : undefined}
                onClick={() => onToggle(p.id, !on)}
              >
                <Avatar name={name} />
                <span>
                  {name}
                  {observer && <small className="muted"> {t('cardPop.observer')}</small>}
                </span>
                {on && (
                  <span className="tick">
                    <Icon name="check" />
                  </span>
                )}
              </button>
            )
          })}
      </div>
    </>
  )
}

type LabelEdit = { id: number | null; name: string; color: number }

export function LabelsPop({
  labels,
  chosen,
  edit,
  onToggle,
  onSave,
  onDelete,
}: {
  labels: Label[]
  chosen: number[] | null
  edit: boolean
  onToggle: (id: number, on: boolean) => void
  onSave: (l: LabelEdit) => void
  onDelete: (id: number) => void
}) {
  const [q, setQ] = useState('')
  const [editing, setEditing] = useState<LabelEdit | null>(null)

  if (editing)
    return (
      <form
        onSubmit={(e: FormEvent) => {
          e.preventDefault()
          onSave(editing)
          setEditing(null)
        }}
      >
        <div className="lrow">
          <span className="lbx" style={labelStyle(editing.color)}>
            {editing.name}
          </span>
        </div>
        <div className="field">
          <label htmlFor="le-n">{t('cardPop.labelName')}</label>
          <input type="text" id="le-n" value={editing.name} onChange={(e) => setEditing({ ...editing, name: e.target.value })} data-focus />
        </div>
        <div className="field">
          <label id="le-c">{t('project.color')}</label>
          <div className="swatches" role="group" aria-labelledby="le-c">
            {labelColors.map((c, i) => (
              <button
                key={c}
                type="button"
                className="sw"
                style={{ background: c }}
                aria-pressed={editing.color === i}
                aria-label={t('cardPop.colorN', { n: i + 1 })}
                onClick={() => setEditing({ ...editing, color: i })}
              />
            ))}
          </div>
        </div>
        <div className="row" style={{ marginTop: 12 }}>
          <button type="submit" className="btn primary sm">
            {t('boardMenu.save')}
          </button>
          {editing.id !== null && (
            <button
              type="button"
              className="btn danger sm"
              onClick={() => {
                onDelete(editing.id!)
                setEditing(null)
              }}
            >
              {t('manage.delete')}
            </button>
          )}
        </div>
      </form>
    )

  return (
    <>
      <input type="search" value={q} onChange={(e) => setQ(e.target.value)} placeholder={t('cardPop.searchLabels')} aria-label={t('cardPop.searchLabels')} data-focus />
      <div style={{ marginTop: 8 }}>
        {labels
          .filter((l) => search(q, l.name))
          .map((l) => {
            const on = chosen?.includes(l.id) ?? false
            return (
              <div key={l.id} className="lrow">
                <button
                  type="button"
                  className="lbx"
                  style={labelStyle(l.color)}
                  aria-pressed={chosen ? on : undefined}
                  aria-label={l.name || t('cardPop.colorN', { n: l.color + 1 })}
                  disabled={!chosen || !edit}
                  onClick={() => onToggle(l.id, !on)}
                >
                  {l.name}
                  {on && <Icon name="check" />}
                </button>
                {edit && (
                  <button type="button" className="ibtn sm" aria-label={t('cardPop.editLabel', { name: l.name })} onClick={() => setEditing({ id: l.id, name: l.name, color: l.color })}>
                    <Icon name="pencil" />
                  </button>
                )}
              </div>
            )
          })}
      </div>
      {edit && (
        <button type="button" className="mi" style={{ marginTop: 6 }} onClick={() => setEditing({ id: null, name: '', color: 0 })}>
          <Icon name="plus" />
          {t('cardPop.newLabel')}
        </button>
      )}
    </>
  )
}

function toDateInput(unix: number | null) {
  if (!unix) return ''
  const d = new Date(unix * 1000)
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
}

function toTimeInput(unix: number | null) {
  if (!unix) return '12:00'
  const d = new Date(unix * 1000)
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}

function fromInputs(date: string, time: string): number | null {
  if (!date) return null
  const [y, m, d] = date.split('-').map(Number)
  const [hh, mm] = (time || '12:00').split(':').map(Number)
  return Math.floor(new Date(y, m - 1, d, hh, mm).getTime() / 1000)
}

export function DatesPop({
  start,
  due,
  onSave,
}: {
  start: number | null
  due: number | null
  onSave: (v: { start_at: number | null; due_at: number | null }) => void
}) {
  const [s, setS] = useState(toDateInput(start))
  const [d, setD] = useState(toDateInput(due))
  const [tm, setTm] = useState(toTimeInput(due))
  return (
    <>
      <div className="field">
        <label htmlFor="dt-s">{t('cardPop.start')}</label>
        <input type="date" id="dt-s" value={s} onChange={(e) => setS(e.target.value)} data-focus />
      </div>
      <div className="field">
        <label htmlFor="dt-d">{t('cardPop.due')}</label>
        <div className="row" style={{ flexWrap: 'nowrap' }}>
          <input type="date" id="dt-d" value={d} onChange={(e) => setD(e.target.value)} />
          <input type="time" value={tm} onChange={(e) => setTm(e.target.value)} style={{ width: 110 }} aria-label={t('cardPop.dueTime')} />
        </div>
      </div>
      <div className="row" style={{ marginTop: 12 }}>
        <button type="button" className="btn primary sm" onClick={() => onSave({ start_at: fromInputs(s, '00:00'), due_at: fromInputs(d, tm) })}>
          {t('boardMenu.save')}
        </button>
        <button type="button" className="btn sm" onClick={() => onSave({ start_at: null, due_at: null })}>
          {t('common.remove')}
        </button>
      </div>
    </>
  )
}

export function CoverPop({ cover, onPick }: { cover: string | null; onPick: (c: string | null) => void }) {
  return (
    <>
      <div className="swatches" style={{ margin: '8px 0' }}>
        {labelColors.map((c) => (
          <button key={c} type="button" className="sw" style={{ background: c }} aria-pressed={cover === c} aria-label={t('cardPop.coverColor', { color: c })} onClick={() => onPick(c)} />
        ))}
      </div>
      <button type="button" className="btn sm" style={{ marginTop: 10 }} onClick={() => onPick(null)}>
        {t('cardPop.removeCover')}
      </button>
    </>
  )
}

export function MovePop({ lists, current, label, onMove }: { lists: ListData[]; current: number; label: string; onMove: (list: number, top: boolean) => void }) {
  const [list, setList] = useState(current)
  const [pos, setPos] = useState<'top' | 'bottom'>('bottom')
  return (
    <>
      <div className="field">
        <label htmlFor="mv-l">{t('cardPop.list')}</label>
        <select id="mv-l" value={list} onChange={(e) => setList(Number(e.target.value))} style={{ width: '100%' }} data-focus>
          {lists.map((l) => (
            <option key={l.id} value={l.id}>
              {l.title}
            </option>
          ))}
        </select>
      </div>
      <div className="field">
        <label htmlFor="mv-p">{t('list.position')}</label>
        <select id="mv-p" value={pos} onChange={(e) => setPos(e.target.value as 'top' | 'bottom')} style={{ width: '100%' }}>
          <option value="top">{t('cardPop.top')}</option>
          <option value="bottom">{t('cardPop.bottom')}</option>
        </select>
      </div>
      <button type="button" className="btn primary sm" onClick={() => onMove(list, pos === 'top')}>
        {label}
      </button>
    </>
  )
}

export function CopyPop({ title, lists, current, onCopy }: { title: string; lists: ListData[]; current: number; onCopy: (title: string, list: number) => void }) {
  const [name, setName] = useState(title)
  const [list, setList] = useState(current)
  return (
    <>
      <div className="field">
        <label htmlFor="cp-t">{t('card.title')}</label>
        <input type="text" id="cp-t" value={name} onChange={(e) => setName(e.target.value)} data-focus />
      </div>
      <div className="field">
        <label htmlFor="cp-l">{t('cardPop.toList')}</label>
        <select id="cp-l" value={list} onChange={(e) => setList(Number(e.target.value))} style={{ width: '100%' }}>
          {lists.map((l) => (
            <option key={l.id} value={l.id}>
              {l.title}
            </option>
          ))}
        </select>
      </div>
      <button type="button" className="btn primary sm" onClick={() => name.trim() && onCopy(name.trim(), list)}>
        {t('cardPop.createCard')}
      </button>
    </>
  )
}

export function TitlePop({ initial, label, onSave }: { initial: string; label: string; onSave: (title: string) => void }) {
  const [v, setV] = useState(initial)
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault()
        if (v.trim()) onSave(v.trim())
      }}
    >
      <div className="field">
        <label htmlFor="tp-t">{t('cardPop.title')}</label>
        <input type="text" id="tp-t" value={v} onChange={(e) => setV(e.target.value)} data-focus />
      </div>
      <button type="submit" className="btn primary sm">
        {label}
      </button>
    </form>
  )
}
