import { t } from '@/i18n'
import { displayName } from '@/ui/names'
import { Avatar } from '@/ui/people'
import { emptyFilter, type Filter } from '@/board/filter'
import type { BoardPerson, Label } from '@/board/types'
import { labelStyle } from '@/card/look'

export function FilterPop({ filter, onChange, people, labels }: { filter: Filter; onChange: (f: Filter) => void; people: BoardPerson[]; labels: Label[] }) {
  const toggle = (key: 'members' | 'labels' | 'due', value: string) => {
    const cur = filter[key]
    onChange({ ...filter, [key]: cur.includes(value) ? cur.filter((x) => x !== value) : [...cur, value] })
  }
  const box = (key: 'members' | 'labels' | 'due', value: string, label: React.ReactNode, name: string) => (
    <label className="fopt" key={`${key}-${value}`}>
      <input type="checkbox" checked={filter[key].includes(value)} onChange={() => toggle(key, value)} aria-label={name} />
      {label}
    </label>
  )

  return (
    <>
      <div className="field">
        <label htmlFor="f-q">{t('filter.keyword')}</label>
        <input type="search" id="f-q" value={filter.q} onChange={(e) => onChange({ ...filter, q: e.target.value })} placeholder={t('filter.keywordHint')} data-focus />
      </div>
      <div className="fsub">{t('filter.members')}</div>
      {box('members', 'none', t('filter.noMembers'), t('filter.noMembers'))}
      {box('members', 'me', t('filter.mine'), t('filter.mine'))}
      {people
        .filter((p) => p.access)
        .map((p) => {
          const name = displayName(p.name, p.email)
          return box(
            'members',
            String(p.id),
            <>
              <Avatar name={name} /> {name}
            </>,
            name,
          )
        })}
      <div className="fsub">{t('filter.labels')}</div>
      {box('labels', 'none', t('filter.noLabels'), t('filter.noLabels'))}
      {labels.map((l) =>
        box(
          'labels',
          String(l.id),
          <span className="lb" style={labelStyle(l.color)}>
            {l.name}
          </span>,
          l.name || t('cardPop.colorN', { n: l.color + 1 }),
        ),
      )}
      <div className="fsub">{t('filter.due')}</div>
      {box('due', 'none', t('filter.noDates'), t('filter.noDates'))}
      {box('due', 'over', t('filter.overdue'), t('filter.overdue'))}
      {box('due', 'day', t('filter.nextDay'), t('filter.nextDay'))}
      {box('due', 'week', t('filter.nextWeek'), t('filter.nextWeek'))}
      {box('due', 'done', t('filter.done'), t('filter.done'))}
      {box('due', 'open', t('filter.open'), t('filter.open'))}
      <button type="button" className="btn sm" style={{ marginTop: 10 }} onClick={() => onChange(emptyFilter)}>
        {t('filter.clearAll')}
      </button>
    </>
  )
}
