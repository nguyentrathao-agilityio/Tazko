import type { BoardPerson } from '@/board/types'
import { dueState, shortDate } from '@/card/look'
import { fromInputs, toDateInput } from '@/card/dates'
import type { ChecklistItem } from '@/card/types'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import { isObserver } from '@/roles'

export type ItemChange = { assignee?: number | null; due_at?: number | null }

export function ItemBadges({ item, people, edit, onOpen }: { item: ChecklistItem; people: BoardPerson[]; edit: boolean; onOpen: (el: Element) => void }) {
  if (!item.assignee && !item.due_at) return null
  const who = people.find((p) => p.id === item.assignee)
  const name = who ? displayName(who.name, who.email) : null
  return (
    <span className="itm">
      {name && (
        <span className="itp" title={t('item.assignedTo', { name })}>
          <Avatar name={name} />
        </span>
      )}
      {item.due_at && (
        <button type="button" className={`itd ${item.done ? '' : dueState(item.due_at, false)}`} disabled={!edit} aria-label={t('item.due', { date: shortDate(item.due_at) })} onClick={(e) => onOpen(e.currentTarget)}>
          <Icon name="clock" />
          {shortDate(item.due_at)}
        </button>
      )}
    </span>
  )
}

export function ItemMetaPop({ item, people, onChange }: { item: ChecklistItem; people: BoardPerson[]; onChange: (change: ItemChange) => void }) {
  return (
    <>
      <div className="sub">{t('item.assignTo')}</div>
      {people
        .filter((p) => p.access && !isObserver(p.role))
        .map((p) => {
          const name = displayName(p.name, p.email)
          const on = item.assignee === p.id
          return (
            <button key={p.id} type="button" className="mr" aria-pressed={on} onClick={() => onChange({ assignee: on ? null : p.id })}>
              <Avatar name={name} />
              <span>{name}</span>
              {on && (
                <span className="tick">
                  <Icon name="check" />
                </span>
              )}
            </button>
          )
        })}
      {item.assignee && (
        <button type="button" className="mr" onClick={() => onChange({ assignee: null })}>
          <span className="muted">{t('item.removeAssignee')}</span>
        </button>
      )}
      <div className="field">
        <label htmlFor={`it-d-${item.id}`}>{t('item.dueDate')}</label>
        <input
          type="date"
          id={`it-d-${item.id}`}
          key={item.due_at ?? 'none'}
          defaultValue={toDateInput(item.due_at)}
          onChange={(e) => e.target.value && onChange({ due_at: fromInputs(e.target.value, '') })}
        />
      </div>
      {item.due_at && (
        <button type="button" className="btn sm" onClick={() => onChange({ due_at: null })}>
          {t('item.removeDue')}
        </button>
      )}
    </>
  )
}
