import { useMemo, useState, type KeyboardEvent } from 'react'
import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { dueState, labelStyle, shortDate } from '@/card/look'
import { ENTER } from '@/keys/keyNames'
import { boardApiOf, cardRoute, ME_API } from '@/paths'
import { useRun } from '@/projects/useRun'
import { EmptyState } from '@/ui/EmptyState'
import { ErrorNote } from '@/ui/ErrorNote'
import { Icon } from '@/ui/Icon'
import type { Go } from '@/ui/Link'
import { Tools } from '@/ui/Tools'
import { useLoad } from '@/ui/useLoad'
import { groupCards, projectKey } from '@/mycards/groups'
import type { MyCard } from '@/mycards/types'

function Row({ card, all, onOpen, onDone }: { card: MyCard; all: boolean; onOpen: () => void; onDone: () => void }) {
  const state = dueState(card.due_at, card.done)
  const where = [all ? card.company.name : null, card.project.name, card.board.name, card.list].filter(Boolean).join(' › ')
  return (
    <div className="mc" role="button" tabIndex={0} onClick={onOpen} onKeyDown={(e: KeyboardEvent) => e.key === ENTER && e.target === e.currentTarget && onOpen()}>
      <button
        type="button"
        className="chk"
        role="checkbox"
        aria-checked={card.done}
        aria-label={t('card.markComplete')}
        onClick={(e) => {
          e.stopPropagation()
          onDone()
        }}
        onKeyDown={(e) => e.stopPropagation()}
      >
        {card.done && <Icon name="check" />}
      </button>
      <div className="mcb">
        <b className={card.done ? 'dn' : ''}>{card.title}</b>
        <small>{where}</small>
      </div>
      <div className="mcm">
        {card.labels.map((l) => (
          <span key={l.name} className="lb" style={labelStyle(l.color)}>
            {l.name}
          </span>
        ))}
        {card.items > 0 && (
          <span className={`bdg${card.items_done === card.items ? ' ok' : ''}`} title={t('card.itemsBadge')}>
            <Icon name="checksq" />
            {card.items_done}/{card.items}
          </span>
        )}
        {card.due_at && (
          <span className={`bdg ${state}`} title={t('card.dates')}>
            <Icon name="clock" />
            {shortDate(card.due_at)}
          </span>
        )}
      </div>
    </div>
  )
}

export function MyCardsPage({ company, go }: { company: Company; go: Go }) {
  const [all, setAll] = useState(false)
  const [q, setQ] = useState('')
  const [project, setProject] = useState('')
  const [hide, setHide] = useState(false)
  const url = all ? `${ME_API}/cards` : `${ME_API}/cards?company=${encodeURIComponent(company.slug)}`
  const { data, error: loadError, reload } = useLoad(() => api<{ cards: MyCard[] }>('GET', url), url)
  const { error, run } = useRun(reload)
  const cards = data?.cards ?? []
  const projects = useMemo(
    () => [...new Map(cards.map((c) => [projectKey(c), all ? `${c.project.name} · ${c.company.name}` : c.project.name])).entries()].sort((a, b) => a[1].localeCompare(b[1])),
    [cards, all],
  )
  const needle = q.trim().toLowerCase()
  const shown = cards.filter((c) => (!project || projectKey(c) === project) && (!hide || !c.done) && (!needle || c.title.toLowerCase().includes(needle)))

  return (
    <>
      <div className="head">
        <div>
          <h1>{t('mycards.title')}</h1>
          <p className="muted" style={{ margin: 0 }}>
            {all ? t('mycards.leadAll') : t('mycards.lead', { company: company.name })}
          </p>
        </div>
        <span className="sp" />
        <Tools />
      </div>
      <div className="tabs2" role="tablist">
        <button type="button" role="tab" aria-selected="true" className="on">
          {t('mycards.assigned', { n: cards.length })}
        </button>
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
      {data && shown.length === 0 && <EmptyState title={t('mycards.noneTitle')} body={t('mycards.noneBody')} />}
      {groupCards(shown).map(([g, list]) => (
        <section key={g} aria-label={t(`mycards.group.${g}`)}>
          <div className={`sechd${g === 'overdue' ? ' bad' : ''}`}>
            {t(`mycards.group.${g}`)} <span className="muted">· {list.length}</span>
          </div>
          <div className="mclist">
            {list.map((c) => (
              <Row
                key={`${c.company.slug}:${c.id}`}
                card={c}
                all={all}
                onOpen={() => go(cardRoute(c.company.slug, c.board.id, c.id))}
                onDone={() => void run(() => api('PATCH', `${boardApiOf(c.company.slug, c.board.id)}/cards/${c.id}`, { done: !c.done }))}
              />
            ))}
          </div>
        </section>
      ))}
    </>
  )
}
