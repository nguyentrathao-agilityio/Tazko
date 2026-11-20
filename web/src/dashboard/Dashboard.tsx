import { useMemo } from 'react'
import type { BoardData, ListData } from '@/board/types'
import { t } from '@/i18n'
import { Avatar } from '@/ui/people'
import { type Bar, boardStats } from '@/dashboard/stats'
import { agingOf } from '@/card/aging'

function Bars({ rows }: { rows: Bar[] }) {
  const most = Math.max(1, ...rows.map((r) => r.value))
  return (
    <>
      {rows.map((r) => (
        <div className="hb" key={r.key}>
          <span className="n">
            {r.person && <Avatar name={r.person} />}
            <span>{r.name}</span>
          </span>
          <div className="tr" role="img" aria-label={`${r.name}: ${r.value}`}>
            <i style={{ width: `${Math.round((r.value / most) * 100)}%`, background: r.color }} />
          </div>
          <span className="v">{r.value}</span>
        </div>
      ))}
    </>
  )
}

export function Dashboard({ board, lists }: { board: BoardData; lists: ListData[] }) {
  const s = useMemo(() => boardStats(lists, board.people, board.labels, (b) => t(`dash.due.${b}`), t('dash.unassigned'), agingOf(board)), [lists, board])
  const stat = (label: string, value: number, color?: string) => (
    <div className="dstat">
      <small>{label}</small>
      <b style={{ color }}>{value}</b>
    </div>
  )

  return (
    <div className="dash">
      <div className="dstats">
        {stat(t('dash.cards'), s.cards)}
        {stat(t('dash.complete'), s.done)}
        {stat(t('dash.overdue'), s.overdue, 'var(--bad)')}
        {stat(t('dash.soon'), s.soon, 'var(--warn)')}
        {board.aging && stat(t('dash.stale'), s.stale)}
      </div>
      <div className="dgrid">
        <section className="dcard" aria-label={t('dash.perList')}>
          <h3>{t('dash.perList')}</h3>
          <Bars rows={s.lists} />
        </section>
        <section className="dcard" aria-label={t('dash.perMember')}>
          <h3>{t('dash.perMember')}</h3>
          <Bars rows={s.members} />
        </section>
        <section className="dcard" aria-label={t('dash.perLabel')}>
          <h3>{t('dash.perLabel')}</h3>
          {s.labels.length ? <Bars rows={s.labels} /> : <p className="muted">{t('filter.noLabels')}</p>}
        </section>
        <section className="dcard" aria-label={t('dash.dueDates')}>
          <h3>{t('dash.dueDates')}</h3>
          <Bars rows={s.due} />
        </section>
        {board.voting && (
          <section className="dcard" aria-label={t('dash.mostVotes')}>
            <h3>{t('dash.mostVotes')}</h3>
            {s.votes.length ? <Bars rows={s.votes} /> : <p className="muted">{t('dash.noVotes')}</p>}
          </section>
        )}
      </div>
    </div>
  )
}
