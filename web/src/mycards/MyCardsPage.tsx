import { useMemo, useState, type KeyboardEvent, type ReactNode } from 'react'
import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { dueState, labelStyle, shortDate, type DueBucket } from '@/card/look'
import { ENTER } from '@/keys/keyNames'
import { boardApiOf, cardRoute, ME_API } from '@/paths'
import { useRun } from '@/projects/useRun'
import { EmptyState } from '@/ui/EmptyState'
import { ErrorNote } from '@/ui/ErrorNote'
import { Icon } from '@/ui/Icon'
import type { Go } from '@/ui/Link'
import { PageHead } from '@/ui/PageHead'
import { Tools } from '@/ui/Tools'
import { useLoad } from '@/ui/useLoad'
import { groupCards, projectKey } from '@/mycards/groups'
import type { MyCard, MyItem } from '@/mycards/types'

function Row({
  done,
  canEdit,
  item,
  title,
  sub,
  due,
  onOpen,
  onDone,
  children,
}: {
  done: boolean
  canEdit: boolean
  item?: boolean
  title: string
  sub: string
  due: number | null
  onOpen: () => void
  onDone: () => void
  children?: ReactNode
}) {
  return (
    <div className="mc" role="button" tabIndex={0} onClick={onOpen} onKeyDown={(e: KeyboardEvent) => e.key === ENTER && e.target === e.currentTarget && onOpen()}>
      <button
        type="button"
        className={item ? 'cbx' : 'chk'}
        role="checkbox"
        aria-checked={done}
        aria-label={item ? t('mycards.markItem') : t('card.markComplete')}
        disabled={!canEdit}
        onClick={(e) => {
          e.stopPropagation()
          onDone()
        }}
        onKeyDown={(e) => e.stopPropagation()}
      >
        {done && <Icon name="check" />}
      </button>
      <div className="mcb">
        <b className={done ? 'dn' : ''}>{title}</b>
        <small>{sub}</small>
      </div>
      <div className="mcm">
        {children}
        {due ? (
          <span className={`bdg ${dueState(due, done)}`} title={t('card.dates')}>
            <Icon name="clock" />
            {shortDate(due)}
          </span>
        ) : null}
      </div>
    </div>
  )
}

function CardBadges({ card }: { card: MyCard }) {
  return (
    <>
      {card.labels.map((l) => (
        <span key={l.id} className="lb" style={labelStyle(l.color)}>
          {l.name}
        </span>
      ))}
      {card.items > 0 && (
        <span className={`bdg${card.items_done === card.items ? ' ok' : ''}`} title={t('card.itemsBadge')}>
          <Icon name="checksq" />
          {card.items_done}/{card.items}
        </span>
      )}
    </>
  )
}

type Tab = 'cards' | 'items'

function Group({ group, count, children }: { group: DueBucket; count: number; children: ReactNode }) {
  return (
    <section aria-label={t(`mycards.group.${group}`)}>
      <div className={`sechd${group === 'overdue' ? ' bad' : ''}`}>
        {t(`mycards.group.${group}`)} <span className="muted">· {count}</span>
      </div>
      <div className="mclist">{children}</div>
    </section>
  )
}

export function MyCardsPage({ company, go }: { company: Company; go: Go }) {
  const [all, setAll] = useState(false)
  const [q, setQ] = useState('')
  const [project, setProject] = useState('')
  const [hide, setHide] = useState(false)
  const [tab, setTab] = useState<Tab>('cards')
  const url = all ? `${ME_API}/cards` : `${ME_API}/cards?company=${encodeURIComponent(company.slug)}`
  const { data, error: loadError, reload } = useLoad(() => api<{ cards: MyCard[]; items?: MyItem[] }>('GET', url), url)
  const { error, run } = useRun(reload)
  const cards = useMemo(() => data?.cards ?? [], [data])
  const items = useMemo(() => data?.items ?? [], [data])
  const projects = useMemo(
    () => [...new Map([...cards, ...items].map((c) => [projectKey(c), all ? `${c.project.name} · ${c.company.name}` : c.project.name])).entries()].sort((a, b) => a[1].localeCompare(b[1])),
    [cards, items, all],
  )
  const needle = q.trim().toLowerCase()
  const keep = (x: MyCard | MyItem, text: string) => (!project || projectKey(x) === project) && (!hide || !x.done) && (!needle || text.toLowerCase().includes(needle))
  const shownCards = cards.filter((c) => keep(c, c.title))
  const shownItems = items.filter((i) => keep(i, i.text))
  const empty = tab === 'cards' ? shownCards.length === 0 : shownItems.length === 0
  const place = (x: { company: { name: string } }, ...parts: string[]) => [all ? x.company.name : null, ...parts].filter(Boolean).join(' › ')

  return (
    <>
      <PageHead title={t('mycards.title')} lead={all ? t('mycards.leadAll') : t('mycards.lead', { company: company.name })}>
        <Tools />
      </PageHead>
      <div className="tabs2" role="tablist">
        {(
          [
            ['cards', t('mycards.assigned', { n: cards.length })],
            ['items', t('mycards.items', { n: items.length })],
          ] as [Tab, string][]
        ).map(([k, label]) => (
          <button key={k} type="button" role="tab" aria-selected={tab === k} className={tab === k ? 'on' : ''} onClick={() => setTab(k)}>
            {label}
          </button>
        ))}
      </div>
      <div className="row mcbar">
        <label className="opt">
          <input
            type="checkbox"
            checked={all}
            onChange={(e) => {
              setAll(e.target.checked)
              setProject('')
            }}
          />
          {t('mycards.allCompanies')}
        </label>
        <input type="search" className="mcq" placeholder={t('mycards.filter')} aria-label={t('mycards.filter')} value={q} onChange={(e) => setQ(e.target.value)} />
        <select aria-label={t('mycards.project')} value={project} onChange={(e) => setProject(e.target.value)}>
          <option value="">{t('mycards.allProjects')}</option>
          {projects.map(([k, name]) => (
            <option key={k} value={k}>
              {name}
            </option>
          ))}
        </select>
        <label className="opt">
          <input type="checkbox" checked={hide} onChange={(e) => setHide(e.target.checked)} />
          {t('mycards.hideDone')}
        </label>
      </div>
      <ErrorNote message={error ?? loadError} />
      {data && empty && <EmptyState title={t('mycards.noneTitle')} body={tab === 'cards' ? t('mycards.noneBody') : t('mycards.noItemsBody')} />}
      {tab === 'cards' &&
        groupCards(shownCards).map(([g, list]) => (
          <Group key={g} group={g} count={list.length}>
            {list.map((c) => (
              <Row
                key={`${c.company.slug}:${c.id}`}
                done={c.done}
                canEdit={c.can_edit}
                title={c.title}
                sub={place(c, c.project.name, c.board.name, c.list)}
                due={c.due_at}
                onOpen={() => go(cardRoute(c.company.slug, c.board.id, c.id))}
                onDone={() => void run(() => api('PATCH', `${boardApiOf(c.company.slug, c.board.id)}/cards/${c.id}`, { done: !c.done }))}
              >
                <CardBadges card={c} />
              </Row>
            ))}
          </Group>
        ))}
      {tab === 'items' &&
        groupCards(shownItems).map(([g, list]) => (
          <Group key={g} group={g} count={list.length}>
            {list.map((i) => (
              <Row
                key={`${i.company.slug}:${i.id}`}
                item
                done={i.done}
                canEdit={i.can_edit}
                title={i.text}
                sub={place(i, t('mycards.itemIn', { card: i.card.title, board: i.board.name }))}
                due={i.due_at}
                onOpen={() => go(cardRoute(i.company.slug, i.board.id, i.card.id))}
                onDone={() => void run(() => api('PATCH', `${boardApiOf(i.company.slug, i.board.id)}/items/${i.id}`, { done: !i.done }))}
              />
            ))}
          </Group>
        ))}
    </>
  )
}
