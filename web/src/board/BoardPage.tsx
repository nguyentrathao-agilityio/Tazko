import { useCallback, useEffect, useState, type FormEvent } from 'react'
import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { Link, type Go } from '@/ui/Link'
import { displayName } from '@/ui/names'
import { anchorOf, type Anchor } from '@/ui/anchor'
import { Popover } from '@/ui/Popover'
import { useToast } from '@/ui/toast'
import { useLoad } from '@/ui/useLoad'
import { background } from '@/projects/look'
import { Stack } from '@/projects/Stack'
import { useRun } from '@/projects/useRun'
import { Access } from '@/board/Access'
import { BoardMenu } from '@/board/BoardMenu'
import { ListColumn, type ListActions } from '@/board/ListColumn'
import type { BoardData, ListData } from '@/board/types'

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

type Pop = { kind: 'access' | 'share' | 'members'; at: Anchor } | null

export function BoardPage({ company, id, go }: { company: Company; id: number; go: Go }) {
  const base = `/api/v1/c/${company.slug}/boards/${id}`
  const { data: board, error: loadError, reload } = useLoad(() => api<BoardData>('GET', base), base)
  const { error, run } = useRun(reload)
  const toast = useToast()
  const [pop, setPop] = useState<Pop>(null)
  const [menu, setMenu] = useState(false)
  const [renaming, setRenaming] = useState(false)
  const [adding, setAdding] = useState(false)
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
          <Link to={`/c/${company.slug}`} go={go} className="btn">
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

  const projectPath = `/c/${company.slug}/p/${board.project.id}`
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
  }

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
    <div className="bd" style={{ background: background(board.background), backgroundSize: 'cover' }}>
      <div className="bt">
        <Link to={projectPath} go={go} className="ibtn">
          <Icon name="back" />
          <span className="sr-only">{t('board.backToProject')}</span>
        </Link>
        <div className="crumb2">
          <Link to={`/c/${company.slug}`} go={go}>
            {company.name}
          </Link>
          <span>/</span>
          <Link to={projectPath} go={go}>
            {board.project.name}
          </Link>
        </div>
        <span className="sp" />
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
                if (e.key === 'Escape') setRenaming(false)
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
        <span className="sp" />
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
        <button type="button" className="ibtn" aria-label={t('boardMenu.title')} onClick={() => setMenu(true)}>
          <Icon name="more" />
        </button>
      </div>
      {board.closed && <div className="ro-banner">{t('board.closedBanner')}</div>}
      {!board.closed && board.role === 'observer' && <div className="ro-banner">{t('board.observerBanner')}</div>}
      {error && (
        <div className="ro-banner" role="alert">
          {error}
        </div>
      )}
      <div className="lists" id="lists">
        {lists.map((l, i) => (
          <ListColumn
            key={l.id}
            list={l}
            index={i}
            total={lists.length}
            count={0}
            edit={edit}
            collapsed={collapsed.ids.includes(l.id)}
            onCollapse={(on) => collapsed.set(l.id, on)}
            actions={actions}
          />
        ))}
        {edit &&
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
                  if (e.key === 'Escape') setAdding(false)
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
          ))}
      </div>
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
      {menu && (
        <BoardMenu
          board={board}
          base={base}
          run={run}
          onClose={() => setMenu(false)}
          onCopied={(copy) => {
            setMenu(false)
            toast(t('board.copied'))
            go(`/c/${company.slug}/b/${copy}`)
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
    void run(() => api('POST', `/api/v1/c/${company.slug}/projects/${board.project.id}/invites`, { email, role: String(f.get('role')) })).then((ok) => {
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
