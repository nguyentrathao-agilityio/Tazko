import { useCallback, useEffect, useState } from 'react'
import { api } from '@/http'
import { t } from '@/i18n'
import { anchorOf, type Anchor } from '@/ui/anchor'
import { Icon } from '@/ui/Icon'
import { Avatar } from '@/ui/people'
import { Popover } from '@/ui/Popover'
import { ago } from '@/card/look'
import { noteText, type Note } from '@/notify/notes'

const delays = [1000, 2000, 5000, 10000]

function useUnread(slug: string): number {
  const [count, setCount] = useState(0)

  useEffect(() => {
    if (typeof WebSocket === 'undefined') return
    let socket: WebSocket | null = null
    let timer: number | undefined
    let ping: number | undefined
    let attempt = 0
    let stopped = false
    const scheme = window.location.protocol === 'https:' ? 'wss' : 'ws'

    const connect = () => {
      if (stopped) return
      const ws = new WebSocket(`${scheme}://${window.location.host}/api/v1/c/${slug}/notifications/live`)
      socket = ws
      ws.onmessage = (e) => {
        try {
          const msg = JSON.parse(String(e.data)) as { type: string; count?: number }
          if (msg.type === 'unread' && typeof msg.count === 'number') {
            attempt = 0
            setCount(msg.count)
          } else if (msg.type === 'closed') stopped = true
        } catch {
          return
        }
      }
      ws.onopen = () => {
        ping = window.setInterval(() => ws.readyState === WebSocket.OPEN && ws.send('ping'), 30_000)
      }
      ws.onclose = () => {
        if (socket !== ws) return
        socket = null
        window.clearInterval(ping)
        if (!stopped) timer = window.setTimeout(connect, delays[Math.min(attempt++, delays.length - 1)])
      }
    }

    connect()
    return () => {
      stopped = true
      window.clearTimeout(timer)
      window.clearInterval(ping)
      const ws = socket
      socket = null
      ws?.close()
    }
  }, [slug])

  return count
}

function kindLabel(kind: Note['kind']): string {
  return { mention: t('notify.kind.mention'), assigned: t('notify.kind.assigned'), due: t('notify.kind.due'), card_added: t('notify.kind.card_added') }[kind]
}

export function Bell({ slug }: { slug: string }) {
  const live = useUnread(slug)
  const [pop, setPop] = useState<Anchor | null>(null)
  const [tab, setTab] = useState<'all' | 'unread'>('all')
  const [notes, setNotes] = useState<Note[] | null>(null)
  const base = `/api/v1/c/${slug}/notifications`

  const load = useCallback(() => {
    api<{ notifications: Note[]; unread: number }>('GET', base).then(
      (r) => setNotes(r.notifications),
      () => setNotes([]),
    )
  }, [base])

  useEffect(() => {
    if (pop) load()
  }, [pop, live, load])

  const close = useCallback(() => setPop(null), [])

  const open = (n: Note) => {
    if (!n.read) void api('POST', `${base}/${n.id}/read`).catch(() => undefined)
    setPop(null)
    window.history.pushState(null, '', `/c/${slug}/b/${n.board.id}/c/${n.card.id}`)
    window.dispatchEvent(new PopStateEvent('popstate'))
  }

  const readAll = () => {
    void api('POST', `${base}/read`).then(load, () => undefined)
  }

  const shown = (notes ?? []).filter((n) => tab === 'all' || !n.read)
  const unread = live
  const label = unread ? t('notify.bellUnread', { n: unread }) : t('notify.bell')
  return (
    <>
      <button type="button" className="ibtn bell" aria-label={label} aria-haspopup="dialog" onClick={(e) => setPop(pop ? null : anchorOf(e.currentTarget))}>
        <Icon name="bell" />
        {unread > 0 && <i className="dot">{unread > 99 ? '99+' : unread}</i>}
      </button>
      {pop && (
        <Popover anchor={{ ...pop, left: pop.left + pop.width - 360 }} title={t('notify.title')} width={360} onClose={close}>
          <div className="row ntabs">
            <button type="button" className="chip" aria-pressed={tab === 'all'} onClick={() => setTab('all')}>
              {t('notify.all')}
            </button>
            <button type="button" className="chip" aria-pressed={tab === 'unread'} onClick={() => setTab('unread')}>
              {t('notify.unread', { n: unread })}
            </button>
            <span className="sp" />
            <button type="button" className="btn sm quiet" onClick={readAll} disabled={!unread}>
              {t('notify.markAll')}
            </button>
          </div>
          {notes === null ? (
            <p className="muted nempty">{t('common.loading')}</p>
          ) : shown.length === 0 ? (
            <p className="muted nempty">{t('notify.empty')}</p>
          ) : (
            shown.map((n) => {
              const { who, what } = noteText(n)
              return (
                <button type="button" key={n.id} className={`nrow${n.read ? '' : ' un'}`} onClick={() => open(n)}>
                  {n.actor ? (
                    <Avatar name={who} />
                  ) : (
                    <span className="av c2">
                      <Icon name="clock" />
                    </span>
                  )}
                  <span className="ntx">
                    {who && <b>{who}</b>} {what} <b>{n.card.title}</b>
                    <small>
                      {ago(n.created_at)} · {kindLabel(n.kind)}
                    </small>
                  </span>
                </button>
              )
            })
          )}
        </Popover>
      )}
    </>
  )
}
