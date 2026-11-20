import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import type { MouseEvent } from 'react'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import { dateRange, dueState, labelStyle } from '@/card/look'
import { RepeatBadge } from '@/card/RepeatBadge'
import type { BoardPerson, CardData, Label } from '@/board/types'
import { fieldText, type FieldDef } from '@/card/fieldTypes'

export function CardTile({
  card,
  base,
  fields = [],
  selected = false,
  edit,
  labels,
  people,
  onOpen,
  onQuickEdit,
  onToggleDone,
  onVote,
}: {
  card: CardData
  base?: string
  fields?: FieldDef[]
  selected?: boolean
  edit: boolean
  labels: Label[]
  people: BoardPerson[]
  onOpen: (card: CardData) => void
  onQuickEdit: (card: CardData, el: HTMLElement) => void
  onToggleDone: (card: CardData) => void
  onVote?: (card: CardData) => void
}) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: `c:${card.id}`, disabled: !edit })
  const style = { transform: CSS.Translate.toString(transform), transition }
  const stop = (e: MouseEvent) => e.stopPropagation()
  const own = labels.filter((l) => card.labels?.includes(l.id))
  const members = people.filter((p) => card.members?.includes(p.id))
  const state = dueState(card.due_at, card.done)
  const items = card.items ?? 0
  const front = fields.filter((f) => f.show_on_card).map((f) => ({ f, text: fieldText(f, card.fields?.[String(f.id)]) })).filter((x) => x.text)
  const hasBadges = front.length > 0 || Boolean(card.due_at || card.start_at || card.recurrence || card.links || (onVote && card.votes) || card.has_description || card.comments || card.attachments || items || members.length)

  return (
    <article
      ref={setNodeRef}
      style={style}
      className={`cd${isDragging ? ' dragging' : ''}${card.done ? ' done' : ''}${selected ? ' kf' : ''}`}
      data-card={card.id}
      aria-label={card.title}
      {...attributes}
      {...listeners}
      tabIndex={0}
      onClick={() => onOpen(card)}

    >
      {card.cover_image && base ? (
        <div className="cv img">
          <img src={`${base}/attachments/${card.cover_image}`} alt="" loading="lazy" draggable={false} />
        </div>
      ) : (
        card.cover && <div className="cv" style={{ background: card.cover }} />
      )}
      <div className="cb">
        {own.length > 0 && (
          <div className="lbs">
            {own.map((l) => (
              <span key={l.id} className="lb" style={labelStyle(l.color)} title={l.name}>
                {l.name}
              </span>
            ))}
          </div>
        )}
        <div className="ct">
          <button
            type="button"
            className="chk"
            role="checkbox"
            aria-checked={Boolean(card.done)}
            aria-label={t('card.markComplete')}
            disabled={!edit}
            onPointerDown={(e) => e.stopPropagation()}
            onClick={(e) => {
              stop(e)
              onToggleDone(card)
            }}
          >
            {card.done && <Icon name="check" />}
          </button>
          <span className="ttl">
            {card.is_template && <span className="tplb">{t('ctpl.badge')}</span>}
            {card.title}
          </span>
        </div>
        {hasBadges && (
          <div className="bdgs">
            {(card.due_at || card.start_at) && (
              <span className={`bdg ${state}`} title={t('card.dates')}>
                <Icon name="clock" />
                {dateRange(card.start_at, card.due_at)}
              </span>
            )}
            {onVote && Boolean(card.votes) && (
              <button
                type="button"
                className={`bdg vote${card.voted ? ' me' : ''}`}
                title={t('vote.count', { n: card.votes ?? 0 })}
                aria-label={card.voted ? t('vote.remove') : t('vote.add')}
                aria-pressed={Boolean(card.voted)}
                disabled={!edit}
                onPointerDown={(e) => e.stopPropagation()}
                onClick={(e) => {
                  stop(e)
                  onVote(card)
                }}
              >
                <Icon name="up" />
                {card.votes}
              </button>
            )}
            <RepeatBadge rule={card.recurrence} />
            {card.blocked ? (
              <span className="bdg bad" title={t('links.blockedHint')}>
                {t('links.blocked')}
              </span>
            ) : card.links ? (
              <span className="bdg" title={t('links.badge')}>
                <Icon name="link" />
                {card.links}
              </span>
            ) : null}
            {card.has_description && (
              <span className="bdg" title={t('card.hasDescription')}>
                <Icon name="text" />
              </span>
            )}
            {Boolean(card.comments) && (
              <span className="bdg" title={t('card.commentsBadge')}>
                <Icon name="msg" />
                {card.comments}
              </span>
            )}
            {Boolean(card.attachments) && (
              <span className="bdg" title={t('attach.badge')}>
                <Icon name="clip" />
                {card.attachments}
              </span>
            )}
            {front.map(({ f, text }) => (
              <span key={f.id} className="fld">
                <b>{f.name}</b> {text}
              </span>
            ))}
            {items > 0 && (
              <span className={`bdg${card.items_done === items ? ' ok' : ''}`} title={t('card.itemsBadge')}>
                <Icon name="checksq" />
                {card.items_done}/{items}
              </span>
            )}
            {members.length > 0 && (
              <span className="mm">
                {members.map((m) => (
                  <span key={m.id} title={displayName(m.name, m.email)}>
                    <Avatar name={displayName(m.name, m.email)} />
                  </span>
                ))}
              </span>
            )}
          </div>
        )}
      </div>
      {edit && (
        <button type="button" className="ibtn sm qe" aria-label={t('card.quickEdit')} onClick={(e) => {
          stop(e)
          onQuickEdit(card, e.currentTarget)
        }} onPointerDown={(e) => e.stopPropagation()}>
          <Icon name="pencil" />
        </button>
      )}
    </article>
  )
}
