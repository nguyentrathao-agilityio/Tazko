import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import type { MouseEvent } from 'react'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import type { CardData } from '@/board/types'

export function CardTile({
  card,
  edit,
  onOpen,
  onQuickEdit,
}: {
  card: CardData
  edit: boolean
  onOpen: (card: CardData) => void
  onQuickEdit: (card: CardData, el: HTMLElement) => void
}) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: `c:${card.id}`, disabled: !edit })
  const style = { transform: CSS.Translate.toString(transform), transition }
  const quick = (e: MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation()
    onQuickEdit(card, e.currentTarget)
  }

  return (
    <article
      ref={setNodeRef}
      style={style}
      className={`cd${isDragging ? ' dragging' : ''}`}
      data-card={card.id}
      aria-label={card.title}
      {...attributes}
      {...listeners}
      tabIndex={0}
      onClick={() => onOpen(card)}
      onKeyDown={(e) => {
        listeners?.onKeyDown?.(e)
        if (e.key === 'Enter' && e.target === e.currentTarget) onOpen(card)
      }}
    >
      <div className="cb">
        <div className="ct">
          <span className="ttl">{card.title}</span>
        </div>
      </div>
      {edit && (
        <button type="button" className="ibtn sm qe" aria-label={t('card.quickEdit')} onClick={quick} onPointerDown={(e) => e.stopPropagation()}>
          <Icon name="pencil" />
        </button>
      )}
    </article>
  )
}
