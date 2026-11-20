import { useState, type CSSProperties, type FormEvent, type HTMLAttributes, type ReactNode, type Ref } from 'react'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { anchorOf, type Anchor } from '@/ui/anchor'
import { Popover } from '@/ui/Popover'
import type { ListData } from '@/board/types'
import { ESCAPE } from '@/keys/keyNames'

export type ListActions = {
  rename: (list: ListData, title: string) => void
  copy: (list: ListData, title: string) => void
  moveTo: (list: ListData, index: number) => void
  archive: (list: ListData) => void
  addCard: (list: ListData) => void
  sort: (list: ListData, by: 'name' | 'new' | 'old' | 'votes') => void
  votes?: boolean
  moveAll: (list: ListData, to: ListData) => void
  archiveAll: (list: ListData) => void
}

export type Sortable = {
  ref: Ref<HTMLElement>
  style: CSSProperties
  handle: HTMLAttributes<HTMLElement>
  dragging: boolean
}

type MenuPage = 'main' | 'move' | 'copy' | 'sort' | 'moveall'

export function ListColumn({
  list,
  lists,
  index,
  count,
  total,
  holdsSelection = false,
  edit,
  collapsed,
  onCollapse,
  actions,
  sortable,
  children,
  footer,
}: {
  list: ListData
  lists: ListData[]
  index: number
  count: number
  total?: number
  holdsSelection?: boolean
  edit: boolean
  collapsed: boolean
  onCollapse: (on: boolean) => void
  actions: ListActions
  sortable?: Sortable
  children?: ReactNode
  footer?: ReactNode
}) {
  const [renaming, setRenaming] = useState(false)
  const [menu, setMenu] = useState<{ at: Anchor; page: MenuPage } | null>(null)

  if (collapsed)
    return (
      <section className="ls collapsed" data-list={list.id} ref={sortable?.ref} style={sortable?.style}>
        <button type="button" onClick={() => onCollapse(false)} aria-label={t('list.expand', { title: list.title })}>
          {list.title} · {total ?? count}
        </button>
      </section>
    )

  const saveTitle = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const title = String(new FormData(e.currentTarget).get('title') ?? '').trim()
    setRenaming(false)
    if (title && title !== list.title) actions.rename(list, title)
  }

  return (
    <section
      className={`ls${count ? '' : ' el'}${holdsSelection ? ' kl' : ''}${sortable?.dragging ? ' dragging' : ''}`}
      data-list={list.id}
      aria-label={list.title}
      ref={sortable?.ref}
      style={sortable?.style}
    >
      <header className="lh" {...(edit ? sortable?.handle : {})}>
        {renaming ? (
          <form onSubmit={saveTitle} style={{ flex: 1 }}>
            <input
              name="title"
              className="inl-i"
              defaultValue={list.title}
              aria-label={t('list.title')}
              autoFocus
              onBlur={(e) => e.currentTarget.form?.requestSubmit()}
              onKeyDown={(e) => {
                if (e.key === ESCAPE) {
                  e.stopPropagation()
                  setRenaming(false)
                }
              }}
            />
          </form>
        ) : (
          <h2 style={{ flex: 1, margin: 0, minWidth: 0, display: 'flex' }}>
            <button type="button" className="lt" onClick={() => edit && setRenaming(true)} disabled={!edit} style={{ cursor: edit ? 'pointer' : 'default' }}>
              {list.title}
            </button>
          </h2>
        )}
        <span className="cnt">{total === undefined ? count : `${count}/${total}`}</span>
        <button type="button" className="ibtn sm" onClick={() => onCollapse(true)} aria-label={t('list.collapse')}>
          <Icon name="chev" />
        </button>
        {edit && (
          <button type="button" className="ibtn sm" aria-label={t('list.actions')} onClick={(e) => setMenu({ at: anchorOf(e.currentTarget), page: 'main' })}>
            <Icon name="more" />
          </button>
        )}
      </header>
      <div className="cards">{count > 0 ? children : <div className="empty-l">{total === undefined ? t('list.empty') : t('filter.noMatches')}</div>}</div>
      {footer}
      {menu && (
        <ListMenu
          list={list}
          lists={lists}
          index={index}
          anchor={menu.at}
          page={menu.page}
          setPage={(page) => setMenu({ ...menu, page })}
          onClose={() => setMenu(null)}
          actions={actions}
        />
      )}
    </section>
  )
}

function ListMenu({
  list,
  lists,
  index,
  anchor,
  page,
  setPage,
  onClose,
  actions,
}: {
  list: ListData
  lists: ListData[]
  index: number
  anchor: Anchor
  page: MenuPage
  setPage: (p: MenuPage) => void
  onClose: () => void
  actions: ListActions
}) {
  const done = (fn: () => void) => () => {
    fn()
    onClose()
  }
  if (page === 'move')
    return (
      <Popover anchor={anchor} title={t('list.move')} onClose={onClose} onBack={() => setPage('main')}>
        <form
          onSubmit={(e) => {
            e.preventDefault()
            actions.moveTo(list, Number(new FormData(e.currentTarget).get('pos')))
            onClose()
          }}
        >
          <div className="field">
            <label htmlFor="lm-p">{t('list.position')}</label>
            <select id="lm-p" name="pos" defaultValue={index} style={{ width: '100%' }}>
              {lists.map((_, i) => (
                <option key={i} value={i}>
                  {i + 1}
                  {i === index ? ` ${t('list.current')}` : ''}
                </option>
              ))}
            </select>
          </div>
          <button type="submit" className="btn primary sm">
            {t('list.moveButton')}
          </button>
        </form>
      </Popover>
    )
  if (page === 'copy')
    return (
      <Popover anchor={anchor} title={t('list.copy')} onClose={onClose} onBack={() => setPage('main')}>
        <form
          onSubmit={(e) => {
            e.preventDefault()
            const title = String(new FormData(e.currentTarget).get('title') ?? '').trim()
            if (title) actions.copy(list, title)
            onClose()
          }}
        >
          <div className="field">
            <label htmlFor="lc-t">{t('list.title')}</label>
            <input type="text" id="lc-t" name="title" defaultValue={list.title} />
          </div>
          <button type="submit" className="btn primary sm">
            {t('list.copyButton')}
          </button>
        </form>
      </Popover>
    )
  if (page === 'sort')
    return (
      <Popover anchor={anchor} title={t('list.sortTitle')} onClose={onClose} onBack={() => setPage('main')}>
        <button type="button" className="mi" onClick={done(() => actions.sort(list, 'new'))}>
          {t('list.sortNew')}
        </button>
        <button type="button" className="mi" onClick={done(() => actions.sort(list, 'old'))}>
          {t('list.sortOld')}
        </button>
        <button type="button" className="mi" onClick={done(() => actions.sort(list, 'name'))}>
          {t('list.sortName')}
        </button>
        {actions.votes && (
          <button type="button" className="mi" onClick={done(() => actions.sort(list, 'votes'))}>
            {t('list.sortVotes')}
          </button>
        )}
      </Popover>
    )
  if (page === 'moveall') {
    const others = lists.filter((l) => l.id !== list.id)
    return (
      <Popover anchor={anchor} title={t('list.moveAllTitle')} onClose={onClose} onBack={() => setPage('main')}>
        {others.map((l) => (
          <button key={l.id} type="button" className="mi" onClick={done(() => actions.moveAll(list, l))}>
            {l.title}
          </button>
        ))}
        {others.length === 0 && <p className="hint">{t('list.noOtherLists')}</p>}
      </Popover>
    )
  }
  return (
    <Popover anchor={anchor} title={t('list.actions')} width={290} onClose={onClose}>
      <button type="button" className="mi" onClick={done(() => actions.addCard(list))}>
        <Icon name="plus" />
        {t('list.addCard')}
      </button>
      <button type="button" className="mi" onClick={() => setPage('copy')}>
        <Icon name="copy" />
        {t('list.copy')}
      </button>
      <button type="button" className="mi" onClick={() => setPage('move')}>
        <Icon name="move" />
        {t('list.move')}
      </button>
      <button type="button" className="mi" onClick={() => setPage('sort')}>
        <Icon name="list" />
        {t('list.sort')}
      </button>
      <button type="button" className="mi" onClick={() => setPage('moveall')}>
        <Icon name="right" />
        {t('list.moveAll')}
      </button>
      <button type="button" className="mi" onClick={done(() => actions.archiveAll(list))}>
        <Icon name="archive" />
        {t('list.archiveAll')}
      </button>
      <button type="button" className="mi danger" onClick={done(() => actions.archive(list))}>
        <Icon name="archive" />
        {t('list.archive')}
      </button>
    </Popover>
  )
}
