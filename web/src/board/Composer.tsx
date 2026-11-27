import { useMemo, useState, type FormEvent, type KeyboardEvent } from 'react'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { splitTitles } from '@/board/titles'
import { ENTER, ESCAPE } from '@/keys/keyNames'
import { applySuggestion, parseLine, suggest, type QuickCard } from '@/board/quickadd'
import type { BoardPerson, Label } from '@/board/types'
import { labelStyle, shortDate } from '@/card/look'
import { displayName } from '@/ui/names'

export function Composer({
  open,
  onOpen,
  onClose,
  onAdd,
  onTemplate,
  labels = [],
  people = [],
}: {
  open: boolean
  onOpen: () => void
  onClose: () => void
  onAdd: (cards: QuickCard[]) => Promise<boolean>
  onTemplate?: (el: Element) => void
  labels?: Label[]
  people?: BoardPerson[]
}) {
  const [text, setText] = useState('')
  const parsed = useMemo(() => splitTitles(text).map((line) => parseLine(line, labels, people)), [text, labels, people])
  const hints = useMemo(() => suggest(text, labels, people), [text, labels, people])
  const untitled = parsed.some((p) => !p.title)

  const submit = (e?: FormEvent) => {
    e?.preventDefault()
    if (untitled || parsed.length === 0) return
    const cards = parsed.map(({ title, labels: l, members, due_at }) => ({ title, labels: l, members, due_at }))
    void onAdd(cards).then((ok) => {
      if (ok) setText('')
    })
  }

  const key = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Tab' && hints.length) {
      e.preventDefault()
      setText(applySuggestion(text, hints[0]))
      return
    }

    if (e.key === ENTER && !e.shiftKey) {
      e.preventDefault()
      submit()
    }

    if (e.key === ESCAPE) {
      e.stopPropagation()
      onClose()
    }
  }

  if (!open)
    return (
      <div className="lf">
        <button type="button" className="addc" onClick={onOpen}>
          <Icon name="plus" />
          {t('card.add')}
        </button>
        {onTemplate && (
          <button type="button" className="ibtn sm" aria-label={t('ctpl.button')} title={t('ctpl.buttonHint')} onClick={(e) => onTemplate(e.currentTarget)}>
            <Icon name="tmpl" />
          </button>
        )}
      </div>
    )

  const lines = parsed.length
  const one = lines === 1 ? parsed[0] : null
  const name = (id: number) => {
    const p = people.find((x) => x.id === id)
    return p ? displayName(p.name, p.email) : ''
  }
  return (
    <form className="cmp" onSubmit={submit}>
      <textarea value={text} onChange={(e) => setText(e.target.value)} onKeyDown={key} placeholder={t('card.titlePlaceholder')} aria-label={t('card.title')} autoFocus />
      {hints.length > 0 && (
        <div className="qa-sug" role="listbox" aria-label={t('quick.suggestions')}>
          {hints.map((h, i) => (
            <button key={`${i}-${h.insert}`} type="button" role="option" aria-selected={i === 0} className="qa-chip" onMouseDown={(e) => e.preventDefault()} onClick={() => setText(applySuggestion(text, h))}>
              {h.color !== undefined && <span className="qa-dot" style={labelStyle(h.color)} />}
              {h.label}
            </button>
          ))}
        </div>
      )}
      {one && (one.labels.length > 0 || one.members.length > 0 || one.due_at !== null) && (
        <div className="qa-sum" aria-label={t('quick.preview')}>
          {one.labels.map((id) => {
            const l = labels.find((x) => x.id === id)
            return l ? (
              <span key={`l${id}`} className="lb" style={labelStyle(l.color)}>
                {l.name}
              </span>
            ) : null
          })}
          {one.members.map((id) => (
            <span key={`m${id}`} className="qa-chip">
              @{name(id)}
            </span>
          ))}
          {one.due_at !== null && <span className="qa-chip">{t('quick.due', { when: shortDate(one.due_at) })}</span>}
        </div>
      )}
      {parsed.some((p) => p.unknown.length > 0) && (
        <p className="hint" style={{ padding: '4px 0 0', border: 0 }}>
          {parsed
            .flatMap((p) => p.unknown)
            .map((u) => t(`quick.unknown.${u.kind}`, { text: u.text }))
            .join(' ')}
        </p>
      )}
      {untitled && text.trim() && <p className="hint" style={{ padding: '4px 0 0', border: 0 }}>{t('quick.noTitle')}</p>}
      {lines > 1 && <p className="hint" style={{ padding: '4px 0 0', border: 0 }}>{t('card.manyLines', { n: lines })}</p>}
      {!text && (labels.length > 0 || people.length > 0) && <p className="hint qa-tip">{t('quick.hint')}</p>}
      <div className="row">
        <button type="submit" className="btn primary sm">
          {lines > 1 ? t('card.addMany', { n: lines }) : t('card.addButton')}
        </button>
        <button type="button" className="ibtn sm" aria-label={t('common.cancel')} onClick={onClose}>
          <Icon name="x" />
        </button>
      </div>
    </form>
  )
}
