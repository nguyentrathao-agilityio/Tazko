import { useState, type FormEvent, type ReactNode } from 'react'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { anchorOf, type Anchor } from '@/ui/anchor'
import { Popover } from '@/ui/Popover'
import type { ListData } from '@/board/types'

export type ListActions = {
  rename: (list: ListData, title: string) => void
  copy: (list: ListData, title: string) => void
  moveTo: (list: ListData, index: number) => void
  archive: (list: ListData) => void
}

export function ListColumn({
  list,
  index,
  total,
  count,
  edit,
  collapsed,
  onCollapse,
  actions,
  children,
  footer,
}: {
  list: ListData
  index: number
  total: number
  count: number
  edit: boolean
  collapsed: boolean
  onCollapse: (on: boolean) => void
  actions: ListActions
  children?: ReactNode
  footer?: ReactNode
}) {
  const [renaming, setRenaming] = useState(false)
  const [menu, setMenu] = useState<{ at: Anchor; page: 'main' | 'move' | 'copy' } | null>(null)

  if (collapsed)
    return (
      <section className="ls collapsed" data-list={list.id}>
        <button type="button" onClick={() => onCollapse(false)} aria-label={t('list.expand', { title: list.title })}>
          {list.title} · {count}
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
    <section className={`ls${count ? '' : ' el'}`} data-list={list.id} aria-label={list.title}>
      <header className="lh">
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
                if (e.key === 'Escape') {
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
        <span className="cnt">{count}</span>
        <button type="button" className="ibtn sm" onClick={() => onCollapse(true)} aria-label={t('list.collapse')}>
          <Icon name="chev" />
        </button>
        {edit && (
          <button type="button" className="ibtn sm" aria-label={t('list.actions')} onClick={(e) => setMenu({ at: anchorOf(e.currentTarget), page: 'main' })}>
            <Icon name="more" />
          </button>
        )}
      </header>
      <div className="cards">{children ?? <div className="empty-l">{t('list.empty')}</div>}</div>
      {footer}
      {menu && (
        <ListMenu
          list={list}
          index={index}
          total={total}
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
  index,
  total,
  anchor,
  page,
  setPage,
  onClose,
  actions,
}: {
  list: ListData
  index: number
  total: number
  anchor: Anchor
  page: 'main' | 'move' | 'copy'
  setPage: (p: 'main' | 'move' | 'copy') => void
  onClose: () => void
  actions: ListActions
}) {
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
              {Array.from({ length: total }, (_, i) => (
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
  return (
    <Popover anchor={anchor} title={t('list.actions')} width={290} onClose={onClose}>
      <button type="button" className="mi" onClick={() => setPage('copy')}>
        <Icon name="copy" />
        {t('list.copy')}
      </button>
      <button type="button" className="mi" onClick={() => setPage('move')}>
        <Icon name="move" />
        {t('list.move')}
      </button>
      <button
        type="button"
        className="mi danger"
        onClick={() => {
          actions.archive(list)
          onClose()
        }}
      >
        <Icon name="archive" />
        {t('list.archive')}
      </button>
    </Popover>
  )
}
