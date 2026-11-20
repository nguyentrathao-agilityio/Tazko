import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent } from 'react'
import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { appendAfter } from '@/board/order'
import { Icon } from '@/ui/Icon'
import { Link, type Go } from '@/ui/Link'
import { displayName } from '@/ui/names'
import { anchorOf, type Anchor } from '@/ui/anchor'
import { Popover } from '@/ui/Popover'
import { CardTemplatePop } from '@/templates/CardTemplatePop'
import { useToast } from '@/ui/toast'
import { useLoad } from '@/ui/useLoad'
import { background } from '@/projects/look'
import { Stack } from '@/projects/Stack'
import { useRun } from '@/projects/useRun'
import { Access } from '@/board/Access'
import { BoardMenu } from '@/board/BoardMenu'
import { BoardLists } from '@/board/BoardLists'
import type { ListActions } from '@/board/ListColumn'
import type { BoardData, CardData, ListData } from '@/board/types'
import { CardDialog } from '@/card/CardDialog'
import { active, applyFilter, emptyFilter, toggleMine, type Filter } from '@/board/filter'
import { FilterPop } from '@/board/FilterPop'
import { Tools } from '@/ui/Tools'
import { isTyping, openSearchEvent, popoverOpen } from '@/ui/typing'
import { useBoardKeys } from '@/keys/useBoardKeys'
import { Shortcuts } from '@/keys/Shortcuts'
import { saveSettings, useSettings } from '@/settings/store'
import { ZenStrip } from '@/zen/ZenStrip'
import { Dashboard } from '@/dashboard/Dashboard'
import { TableView } from '@/table/TableView'
import { useLive } from '@/live/useLive'
import { LiveList, LivePill } from '@/live/LivePill'
import { DatesPop, LabelsPop, MembersPop, MovePop } from '@/card/Pops'
import { boardApiOf, boardRoute, cardRouteUnder, companyRoute, projectApi, projectRoute } from '@/paths'
import { ENTER, ESCAPE } from '@/keys/keyNames'

function useCollapsed(boardId: number) {
  const key = `tazko.collapsed.${boardId}`
  const [ids, setIds] = useState<number[]>(() => {
    try {
      return JSON.parse(window.localStorage.getItem(key) ?? '[]') as number[]
    } catch {
      return []
    }
  })
  const set = useCallback(
    (id: number, on: boolean) =>
      setIds((cur) => {
        const next = on ? [...cur.filter((x) => x !== id), id] : cur.filter((x) => x !== id)
        try {
          window.localStorage.setItem(key, JSON.stringify(next))
        } catch {
          return next
        }
        return next
      }),
    [key],
  )
  return { ids, set }
}

type CardPop = 'quick' | 'k-members' | 'k-dates' | 'k-labels' | 'k-move'

type Pop = { kind: 'access' | 'share' | 'members' | 'filter' | 'live'; at: Anchor } | { kind: 'cardtpl'; at: Anchor; list: number } | { kind: CardPop; at: Anchor; card: CardData } | null

export type BoardView = 'board' | 'table' | 'dashboard'

export function BoardPage({ company, id, cardId, me, go, view = 'board' }: { company: Company; id: number; cardId: number | null; me: number; go: Go; view?: BoardView }) {
  const base = boardApiOf(company.slug, id)
  const { data: board, error: loadError, reload } = useLoad(() => api<BoardData>('GET', base), base)
  const { error, run } = useRun(reload)
  const toast = useToast()
  const [pop, setPop] = useState<Pop>(null)
  const [menu, setMenu] = useState(false)
  const [renaming, setRenaming] = useState(false)
  const [adding, setAdding] = useState(false)
  const [composing, setComposing] = useState<number | null>(null)
  const [filter, setFilter] = useState<Filter>(emptyFilter)
  const labelNames = useMemo(() => new Map((board?.labels ?? []).map((l) => [l.id, l.name])), [board])
  const shownLists = useMemo(() => (board ? applyFilter(board.lists, filter, me, labelNames) : []), [board, filter, me, labelNames])

  const settings = useSettings()
  const [tick, setTick] = useState(0)
  const pending = useRef<number | undefined>(undefined)
  const shownCount = shownLists.reduce((n, l) => n + l.cards.length, 0)
  const live = useLive(company.slug, id, board ? (board.last_event ?? 0) : null, () => {
    window.clearTimeout(pending.current)
    pending.current = window.setTimeout(() => {
      reload()
      setTick((n) => n + 1)
    }, 120)
  })
  const zen = settings.zen
  const [help, setHelp] = useState(false)

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() !== 'q' || e.ctrlKey || e.metaKey || e.altKey || isTyping(e.target) || popoverOpen()) return
      e.preventDefault()
      setFilter((f) => toggleMine(f))
    }
    window.addEventListener('keydown', onKey)

    return () => window.removeEventListener('keydown', onKey)
  }, [])

  const setZen = (on: boolean) => {
    void saveSettings({ zen: on }).catch(() => undefined)
    toast(on ? t('zen.on') : t('zen.off'))
  }

  const anchorFor = (card: CardData) => {
    const el = document.querySelector(`[data-card="${card.id}"]`)
    return el ? anchorOf(el) : { left: 200, top: 120, bottom: 120, width: 0 }
  }

  const boardRoot = boardRoute(company.slug, id)
  const boardBase = view === 'board' ? boardRoot : `${boardRoot}/${view}`
  const cardPath = (card: number) => cardRouteUnder(boardBase, card)
  const keys = useBoardKeys({
    lists: view === 'board' ? shownLists : [],
    edit: Boolean(board?.can.edit),
    cardOpen: cardId,
    zen,
    actions: {
      open: (c) => go(cardPath(c.id)),
      goToCard: (c) => go(cardPath(c.id), true),
      quick: (c) => setPop({ kind: 'quick', at: anchorFor(c), card: c }),
      done: (c) => void run(() => api('PATCH', `${base}/cards/${c.id}`, { done: !c.done })),
      pop: (kind, c) => setPop({ kind: `k-${kind}`, at: anchorFor(c), card: c }),
      archive: (c) =>
        void run(() => api('POST', `${base}/cards/${c.id}/archive`)).then((ok) => {
          if (ok) toast(t('card.archived'), () => run(() => api('POST', `${base}/cards/${c.id}/restore`)))
        }),
      label: (c, index) => {
        const label = board?.labels[index]
        if (label) void run(() => api('PUT', `${base}/cards/${c.id}/labels/${label.id}`, { on: !(c.labels ?? []).includes(label.id) }))
      },
      compose: (list) => setComposing(list),
      filter: () => {
        const el = document.querySelector('[data-filter]')
        if (el) setPop({ kind: 'filter', at: anchorOf(el) })
      },
      zen: () => setZen(!zen),
      help: () => setHelp((h) => !h),
      leaveZen: () => {
        if (!zen) return false
        setZen(false)
        return true
      },
    },
  })
  const collapsed = useCollapsed(id)

  useEffect(() => {
    if (board) document.title = `${board.name} · Tazko`
  }, [board])

  if (loadError)
    return (
      <div className="page mx-auto">
        <div className="empty">
          <h2>{t('board.missingTitle')}</h2>
          <p className="muted">{t('board.missingBody')}</p>
          <Link to={companyRoute(company.slug)} go={go} className="btn">
            {t('project.back')}
          </Link>
        </div>
      </div>
    )
  if (!board)
    return (
      <p role="status" className="p-8 text-center text-muted">
        {t('common.loading')}
      </p>
    )

  const projectPath = projectRoute(company.slug, board.project.id)
  const edit = board.can.edit
  const visible = board.restricted ? board.people.filter((p) => p.access) : board.people
  const lists = board.lists

  const actions: ListActions = {
    rename: (l, title) => void run(() => api('PATCH', `${base}/lists/${l.id}`, { title })),
    copy: (l, title) => void run(() => api('POST', `${base}/lists/${l.id}/copy`, { title })),
    archive: (l: ListData) =>
      void run(() => api('POST', `${base}/lists/${l.id}/archive`)).then((ok) => {
        if (ok) toast(t('list.archived', { title: l.title }), () => run(() => api('POST', `${base}/lists/${l.id}/restore`)))
      }),
    moveTo: (l, index) => {
      const others = lists.filter((x) => x.id !== l.id)
      const after = index === 0 ? null : others[Math.min(index, others.length) - 1].id
      void run(() => api('POST', `${base}/lists/${l.id}/move`, { after }))
    },
    addCard: (l) => setComposing(l.id),
    sort: (l, by) => void run(() => api('POST', `${base}/lists/${l.id}/sort`, { by })),
    moveAll: (l, to) => void run(() => api('POST', `${base}/lists/${l.id}/move-cards`, { to: to.id })),
    archiveAll: (l) =>
      void run(() => api('POST', `${base}/lists/${l.id}/archive-cards`)).then((ok) => {
        if (ok) toast(t('list.cardsArchived', { title: l.title }))
      }),
  }

  const addCards = (l: ListData, titles: string[]) =>
    run(() => api('POST', `${base}/cards`, titles.length === 1 ? { list_id: l.id, title: titles[0] } : { list_id: l.id, titles }))

  const rename = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const name = String(new FormData(e.currentTarget).get('name') ?? '').trim()
    setRenaming(false)
    if (name && name !== board.name) void run(() => api('PATCH', base, { name }))
  }

  const addList = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const form = e.currentTarget
    const title = String(new FormData(form).get('title') ?? '').trim()
    if (!title) return
    void run(() => api('POST', `${base}/lists`, { title })).then(() => form.reset())
  }

  return (
    <div className={`bd${zen ? ` zen zd-${settings.zen_density}` : ''}${board.background.startsWith('img:') ? ' img' : ''}`} style={{ background: background(board.background, base), backgroundSize: 'cover' }}>
      {zen && (
        <ZenStrip
          name={board.name}
          mine={filter.members.includes('me')}
          hideDone={Boolean(filter.hideDone)}
          density={settings.zen_density}
          onMine={() => setFilter((f) => toggleMine(f))}
          onHideDone={() => setFilter((f) => ({ ...f, hideDone: !f.hideDone }))}
          onDensity={(d) => void saveSettings({ zen_density: d }).catch(() => undefined)}
          onSearch={() => window.dispatchEvent(new Event(openSearchEvent))}
          onExit={() => setZen(false)}
        />
      )}
      <div className="bt">
        <Link to={projectPath} go={go} className="ibtn">
          <Icon name="back" />
          <span className="sr-only">{t('board.backToProject')}</span>
        </Link>
        <div className="crumb2">
          <Link to={companyRoute(company.slug)} go={go}>
            {company.name}
          </Link>
          <span>/</span>
          <Link to={projectPath} go={go}>
            {board.project.name}
          </Link>
        </div>
        <span className="sp" />
        <Tools />
      </div>
      <div className="bh">
        {renaming ? (
          <form onSubmit={rename}>
            <input
              name="name"
              className="inl-i bname-i"
              defaultValue={board.name}
              aria-label={t('board.title')}
              autoFocus
              onBlur={(e) => e.currentTarget.form?.requestSubmit()}
              onKeyDown={(e) => {
                if (e.key === ESCAPE) setRenaming(false)
              }}
            />
          </form>
        ) : (
          <h1 style={{ margin: 0, display: 'flex' }}>
            <button type="button" className="bname" title={edit ? t('board.rename') : undefined} onClick={() => edit && setRenaming(true)}>
              {board.name}
            </button>
          </h1>
        )}
        <button
          type="button"
          className="ibtn star"
          aria-pressed={board.starred}
          aria-label={t('board.star')}
          title={t('board.star')}
          onClick={() => void run(() => api('PUT', `${base}/star`, { starred: !board.starred }))}
        >
          <Icon name="star" />
        </button>
        <button type="button" className="chip" onClick={(e) => setPop({ kind: 'access', at: anchorOf(e.currentTarget) })}>
          <Icon name={board.restricted ? 'lock' : 'users'} />
          {board.restricted ? t('board.restricted') : t('board.wholeProject')}
        </button>
        <div className="vtabs" role="tablist" aria-label={t('view.label')}>
          {(
            [
              ['board', t('view.board'), boardRoot],
              ['table', t('view.table'), `${boardRoot}/table`],
              ['dashboard', t('view.dashboard'), `${boardRoot}/dashboard`],
            ] as [BoardView, string, string][]
          ).map(([k, label, to]) => (
            <button key={k} type="button" role="tab" aria-selected={view === k} className={view === k ? 'on' : ''} onClick={() => view !== k && go(to)}>
              {label}
            </button>
          ))}
        </div>
        <span className="sp" />
        <LivePill status={live.status} viewers={live.viewers} onClick={(el) => setPop({ kind: 'live', at: anchorOf(el) })} />
        <button
          type="button"
          aria-label={t('board.members')}
          style={{ border: 0, background: 'none', cursor: 'pointer' }}
          onClick={(e) => setPop({ kind: 'members', at: anchorOf(e.currentTarget) })}
        >
          <Stack people={visible} />
        </button>
        {board.can.invite && (
          <button type="button" className="btn sm" onClick={(e) => setPop({ kind: 'share', at: anchorOf(e.currentTarget) })}>
            <Icon name="share" />
            {t('board.share')}
          </button>
        )}
        <button type="button" className="btn sm" title={t('zen.toggleHint')} onClick={() => setZen(true)}>
          <Icon name="zen" />
          {t('zen.button')}
        </button>
        <button type="button" className="btn sm" data-filter aria-label={t('filter.button')} onClick={(e) => setPop({ kind: 'filter', at: anchorOf(e.currentTarget) })}>
          <Icon name="filter" />
          {t('filter.button')}
          {active(filter) ? ' •' : ''}
        </button>
        <button type="button" className="ibtn" aria-label={t('boardMenu.title')} onClick={() => setMenu(true)}>
          <Icon name="more" />
        </button>
      </div>
      {board.closed && <div className="ro-banner">{t('board.closedBanner')}</div>}
      {!board.closed && board.role === 'observer' && <div className="ro-banner">{t('board.observerBanner')}</div>}
      {active(filter) && (
        <div className="fbar" role="status">
          <Icon name="filter" />
          {view === 'dashboard' ? (
            t('dash.filtered', { n: shownCount })
          ) : (
            <>
              <b>{shownCount}</b> {t('filter.matching')}
            </>
          )}
          <button type="button" onClick={() => setFilter(emptyFilter)}>
            {t('filter.clear')}
          </button>
        </div>
      )}
      {error && (
        <div className="ro-banner" role="alert">
          {error}
        </div>
      )}
      {view === 'table' && (
        <TableView board={board} lists={shownLists} total={lists.reduce((n, l) => n + l.cards.length, 0)} base={base} me={me} run={run} onOpen={(c) => go(cardPath(c.id))} />
      )}
      {view === 'dashboard' && <Dashboard board={board} lists={shownLists} />}
      {view === 'board' && (
        <BoardLists
        lists={shownLists}
        base={base}
        fields={board.fields}
        totals={active(filter) ? new Map(lists.map((l) => [l.id, l.cards.length])) : null}
        edit={edit}
        collapsed={collapsed.ids}
        onCollapse={collapsed.set}
        actions={actions}
        composing={composing}
        setComposing={setComposing}
        onAddCards={addCards}
        onCardTemplate={(l, el) => setPop({ kind: 'cardtpl', at: anchorOf(el), list: l.id })}
        onMoveCard={(m) => void run(() => api('POST', `${base}/cards/${m.card}/move`, { list_id: m.list, after: m.after }))}
        onMoveList={(m) => void run(() => api('POST', `${base}/lists/${m.list}/move`, { after: m.after }))}
        labels={board.labels}
        people={board.people}
        onOpenCard={(c) => go(cardPath(c.id))}
        onToggleDone={(c) => void run(() => api('PATCH', `${base}/cards/${c.id}`, { done: !c.done }))}
        onQuickEdit={(card, el) => setPop({ kind: 'quick', at: anchorOf(el), card })}
        selected={keys.selected}
        after={
          edit &&
          (adding ? (
            <form className="addl open" onSubmit={addList}>
              <input
                type="text"
                name="title"
                className="inl-i"
                placeholder={t('list.newPlaceholder')}
                aria-label={t('list.title')}
                autoFocus
                onKeyDown={(e) => {
                  if (e.key === ESCAPE) setAdding(false)
                }}
              />
              <div className="row">
                <button type="submit" className="btn primary sm">
                  {t('list.add')}
                </button>
                <button type="button" className="ibtn sm" aria-label={t('common.cancel')} onClick={() => setAdding(false)}>
                  <Icon name="x" />
                </button>
              </div>
            </form>
          ) : (
            <button type="button" className="addl" onClick={() => setAdding(true)}>
              <Icon name="plus" />
              {t('list.addAnother')}
            </button>
          ))
        }
      />
      )}
      {pop?.kind === 'quick' && (
        <Popover anchor={pop.at} title={t('card.quickEdit')} width={300} onClose={() => setPop(null)}>
          <QuickEdit
            card={pop.card}
            lists={lists}
            onSave={(title) => {
              setPop(null)
              if (title !== pop.card.title) void run(() => api('PATCH', `${base}/cards/${pop.card.id}`, { title }))
            }}
            onArchive={() => {
              const card = pop.card
              setPop(null)
              void run(() => api('POST', `${base}/cards/${card.id}/archive`)).then((ok) => {
                if (ok) toast(t('card.archived'), () => run(() => api('POST', `${base}/cards/${card.id}/restore`)))
              })
            }}
          />
        </Popover>
      )}
      {pop && pop.kind.startsWith('k-') && 'card' in pop && (
        <Popover anchor={pop.at} title={t(pop.kind === 'k-members' ? 'card.members' : pop.kind === 'k-dates' ? 'card.dates' : pop.kind === 'k-labels' ? 'card.labels' : 'card.moveTitle')} onClose={() => setPop(null)}>
          {pop.kind === 'k-members' && (
            <MembersPop people={board.people} chosen={pop.card.members ?? []} onToggle={(uid, on) => void run(() => api('PUT', `${base}/cards/${pop.card.id}/members/${uid}`, { on }))} />
          )}
          {pop.kind === 'k-labels' && (
            <LabelsPop
              labels={board.labels}
              chosen={pop.card.labels ?? []}
              edit={edit}
              onToggle={(lid, on) => void run(() => api('PUT', `${base}/cards/${pop.card.id}/labels/${lid}`, { on }))}
              onSave={(l) => void run(() => (l.id === null ? api('POST', `${base}/labels`, { name: l.name, color: l.color }) : api('PATCH', `${base}/labels/${l.id}`, { name: l.name, color: l.color })))}
              onDelete={(lid) => void run(() => api('DELETE', `${base}/labels/${lid}`))}
            />
          )}
          {pop.kind === 'k-dates' && (
            <DatesPop
              start={pop.card.start_at ?? null}
              due={pop.card.due_at ?? null}
              onSave={(v) => void run(() => api('PATCH', `${base}/cards/${pop.card.id}`, v)).then(() => setPop(null))}
            />
          )}
          {pop.kind === 'k-move' && (
            <MovePop
              lists={lists}
              current={lists.find((l) => l.cards.some((c) => c.id === pop.card.id))?.id ?? lists[0].id}
              label={t('list.moveButton')}
              onMove={(list, top) => {
                const after = appendAfter(lists, list, pop.card.id, top)
                void run(() => api('POST', `${base}/cards/${pop.card.id}/move`, { list_id: list, after })).then(() => setPop(null))
              }}
            />
          )}
        </Popover>
      )}
      {help && <Shortcuts onClose={() => setHelp(false)} />}
      {pop?.kind === 'cardtpl' && (
        <Popover anchor={pop.at} title={t('ctpl.title')} width={300} onClose={() => setPop(null)}>
          <CardTemplatePop
            base={base}
            list={pop.list}
            onMade={(id) => {
              setPop(null)
              reload()
              go(cardPath(id))
            }}
          />
        </Popover>
      )}
      {pop?.kind === 'live' && (
        <Popover anchor={pop.at} title={t('live.title')} width={300} onClose={() => setPop(null)}>
          <LiveList viewers={live.viewers} me={me} />
        </Popover>
      )}
      {pop?.kind === 'filter' && (
        <Popover anchor={pop.at} title={t('filter.title')} width={340} onClose={() => setPop(null)}>
          <FilterPop filter={filter} onChange={setFilter} people={board.people} labels={board.labels} fields={board.fields} />
        </Popover>
      )}
      {pop?.kind === 'access' && (
        <Popover anchor={pop.at} title={t('boardMenu.access')} width={340} onClose={() => setPop(null)}>
          <Access board={board} base={base} run={run} />
        </Popover>
      )}
      {pop?.kind === 'members' && (
        <Popover anchor={pop.at} title={t('board.members')} onClose={() => setPop(null)}>
          {visible.map((p) => (
            <div key={p.id} className="mr" style={{ cursor: 'default' }}>
              <span>{displayName(p.name, p.email)}</span>
              <span className="pill" style={{ marginLeft: 'auto' }}>
                {t(`projectRole.${p.role}`)}
              </span>
            </div>
          ))}
          <p className="hint" style={{ marginTop: 8, padding: 0, border: 0 }}>
            {board.restricted ? t('board.restrictedNote') : t('board.wholeProjectNote')}
          </p>
        </Popover>
      )}
      {pop?.kind === 'share' && (
        <Popover anchor={pop.at} title={t('board.shareTitle')} onClose={() => setPop(null)}>
          <Share company={company} board={board} onDone={() => setPop(null)} />
        </Popover>
      )}
      {cardId && (
        <CardDialog
          key={cardId}
          me={me}
          board={board}
          base={base}
          cardId={cardId}
          link={cardPath(cardId)}
          onClose={() => go(boardBase)}
          onChanged={reload}
          tick={tick}
          onOpenCard={(c) => go(cardPath(c))}
        />
      )}
      {menu && (
        <BoardMenu
          board={board}
          base={base}
          run={run}
          onClose={() => setMenu(false)}
          onCopied={(copy) => {
            setMenu(false)
            toast(t('board.copied'))
            go(boardRoute(company.slug, copy))
          }}
          onClosed={() => {
            setMenu(false)
            toast(t('board.closedToast', { name: board.name }))
            go(projectPath)
          }}
        />
      )}
    </div>
  )
}

function Share({ company, board, onDone }: { company: Company; board: BoardData; onDone: () => void }) {
  const toast = useToast()
  const { error, run } = useRun()
  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const f = new FormData(e.currentTarget)
    const email = String(f.get('email') ?? '').trim()
    void run(() => api('POST', `${projectApi(company.slug, board.project.id)}/invites`, { email, role: String(f.get('role')) })).then((ok) => {
      if (ok) {
        toast(t('toast.invited', { email: email.toLowerCase() }))
        onDone()
      }
    })
  }

  return (
    <form onSubmit={submit} noValidate>
      <p className="hint" style={{ margin: '0 0 8px', padding: 0, border: 0 }}>
        {t('board.shareHint')}
      </p>
      <div className="field">
        <label htmlFor="sh-e">{t('members.email')}</label>
        <input type="email" id="sh-e" name="email" placeholder="name@example.com" data-focus />
      </div>
      <div className="field">
        <label htmlFor="sh-r">{t('members.role')}</label>
        <select id="sh-r" name="role" style={{ width: '100%' }}>
          <option value="member">{t('projectRole.member')}</option>
          <option value="observer">{t('projectRole.observer')}</option>
          {board.can.manage && <option value="owner">{t('projectRole.owner')}</option>}
        </select>
      </div>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <button type="submit" className="btn primary sm">
        {t('project.inviteButton')}
      </button>
    </form>
  )
}

function QuickEdit({ card, onSave, onArchive }: { card: CardData; lists: ListData[]; onSave: (title: string) => void; onArchive: () => void }) {
  const [title, setTitle] = useState(card.title)
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault()
        if (title.trim()) onSave(title.trim())
      }}
    >
      <textarea
        className="inl-i"
        style={{ minHeight: 70, height: 'auto', padding: 8 }}
        value={title}
        aria-label={t('card.title')}
        data-focus
        onChange={(e) => setTitle(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === ENTER && !e.shiftKey) {
            e.preventDefault()
            if (title.trim()) onSave(title.trim())
          }
        }}
      />
      <div className="row" style={{ margin: '8px 0' }}>
        <button type="submit" className="btn primary sm">
          {t('boardMenu.save')}
        </button>
      </div>
      <button type="button" className="mi danger" onClick={onArchive}>
        <Icon name="archive" />
        {t('card.archive')}
      </button>
    </form>
  )
}
