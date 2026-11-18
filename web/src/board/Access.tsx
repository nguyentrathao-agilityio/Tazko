import { api } from '@/http'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import type { BoardData } from '@/board/types'

export function Access({ board, base, run }: { board: BoardData; base: string; run: (fn: () => Promise<unknown>) => Promise<boolean> }) {
  const manage = board.can.manage
  const chosen = board.people.filter((p) => p.access).map((p) => p.id)
  const save = (restricted: boolean, people: number[]) => void run(() => api('PUT', `${base}/access`, { restricted, people }))
  const toggle = (id: number) => save(true, chosen.includes(id) ? chosen.filter((x) => x !== id) : [...chosen, id])

  return (
    <>
      <p className="hint" style={{ margin: '0 0 8px', padding: 0, border: 0 }}>
        {t('board.accessHint')}
      </p>
      {manage ? (
        <div className="switchrow">
          <div>
            <b>{t('board.restrict')}</b>
            <small>{t('board.restrictHint')}</small>
          </div>
          <button
            type="button"
            className="switchbtn"
            role="switch"
            aria-checked={board.restricted}
            aria-label={t('board.restrict')}
            onClick={() => save(!board.restricted, board.restricted ? [] : chosen)}
          />
        </div>
      ) : (
        <p>
          <b>{board.restricted ? t('board.restrictedNote') : t('board.wholeProjectNote')}</b>
        </p>
      )}
      {board.restricted && (
        <div style={{ marginTop: 8 }}>
          {board.people.map((p) => {
            const name = displayName(p.name, p.email)
            return (
              <button key={p.id} type="button" className="mr" disabled={!manage} aria-pressed={p.access} onClick={() => toggle(p.id)}>
                <Avatar name={name} />
                <span>
                  {name} <small className="muted">{t(`projectRole.${p.role}`)}</small>
                </span>
                {p.access && (
                  <span className="tick">
                    <Icon name="check" />
                  </span>
                )}
              </button>
            )
          })}
        </div>
      )}
    </>
  )
}
