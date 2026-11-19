import type { KeyboardEvent, MouseEvent } from 'react'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { background, boardApi } from '@/projects/look'
import type { BoardTileData } from '@/board/types'

export function BoardTile({ board, onOpen, onStar }: { board: BoardTileData; onOpen: () => void; onStar: (on: boolean) => void }) {
  const star = (e: MouseEvent) => {
    e.stopPropagation()
    onStar(!board.starred)
  }
  const key = (e: KeyboardEvent) => {
    if (e.target === e.currentTarget && (e.key === 'Enter' || e.key === ' ')) {
      e.preventDefault()
      onOpen()
    }
  }

  return (
    <div className="btile" role="link" tabIndex={0} aria-label={board.name} onClick={onOpen} onKeyDown={key} style={{ background: background(board.background, boardApi(board.id)) }}>
      <b>{board.name}</b>
      <small>
        {board.restricted && (
          <>
            <Icon name="lock" /> {t('board.restricted')} ·{' '}
          </>
        )}
        {t('board.tileCounts', { lists: board.lists, cards: board.cards })}
      </small>
      <button type="button" className="ibtn sm star" aria-pressed={board.starred} aria-label={t('board.star')} onClick={star}>
        <Icon name="star" />
      </button>
    </div>
  )
}
