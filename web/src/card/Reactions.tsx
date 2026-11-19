import { useState } from 'react'
import { api } from '@/http'
import { t } from '@/i18n'
import { anchorOf, type Anchor } from '@/ui/anchor'
import { Icon } from '@/ui/Icon'
import { Popover } from '@/ui/Popover'
import { EMOJI, type Reaction } from '@/card/fieldTypes'

export function Reactions({
  comment,
  reactions,
  edit,
  base,
  run,
}: {
  comment: number
  reactions: Reaction[]
  edit: boolean
  base: string
  run: (fn: () => Promise<unknown>) => Promise<boolean>
}) {
  const [pick, setPick] = useState<Anchor | null>(null)
  if (!reactions.length && !edit) return null
  const toggle = (emoji: string, on: boolean) => void run(() => api('PUT', `${base}/comments/${comment}/reactions`, { emoji, on }))

  return (
    <div className="rx">
      {reactions.map((r) => (
        <button
          key={r.emoji}
          type="button"
          className={`rxc${r.mine ? ' me' : ''}`}
          title={r.people.join(', ')}
          aria-pressed={r.mine}
          aria-label={t('react.chip', { emoji: r.emoji, n: r.count })}
          disabled={!edit}
          onClick={() => toggle(r.emoji, !r.mine)}
        >
          <span>{r.emoji}</span>
          {r.count}
        </button>
      ))}
      {edit && (
        <button type="button" className="rxc add" aria-label={t('react.add')} onClick={(e) => setPick(anchorOf(e.currentTarget))}>
          <Icon name="smile" />
        </button>
      )}
      {pick && (
        <Popover anchor={pick} title={t('react.title')} width={270} onClose={() => setPick(null)}>
          <div className="row" style={{ gap: 4 }}>
            {EMOJI.map((e) => (
              <button
                key={e}
                type="button"
                className="ibtn rxpick"
                aria-label={t('react.with', { emoji: e })}
                onClick={() => {
                  setPick(null)
                  toggle(e, !reactions.some((r) => r.emoji === e && r.mine))
                }}
              >
                {e}
              </button>
            ))}
          </div>
        </Popover>
      )}
    </div>
  )
}
