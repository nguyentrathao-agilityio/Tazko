import { useEffect, useState, type KeyboardEvent } from 'react'
import type { Company } from '@/api'
import { api } from '@/http'
import { t } from '@/i18n'
import type { Go } from '@/ui/Link'
import { isTyping, openSearchEvent, popoverOpen } from '@/ui/typing'
import { shortDate } from '@/card/look'
import { background } from '@/projects/look'

type Hit = { id: number; title: string; board: { id: number; name: string; background: string }; list: string; due_at: number | null; archived: boolean; project: string }
type BoardHit = { id: number; name: string; background: string; project: string }
type Result = { boards: BoardHit[]; cards: Hit[]; total: number }

export function SearchHost({ company, go }: { company: Company; go: Go }) {
  const [open, setOpen] = useState(false)

  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key !== '/' || e.ctrlKey || e.metaKey || e.altKey || isTyping(e.target) || popoverOpen()) return
      e.preventDefault()
      setOpen(true)
    }
    const onOpen = () => setOpen(true)
    window.addEventListener('keydown', onKey)
    window.addEventListener(openSearchEvent, onOpen)

    return () => {
      window.removeEventListener('keydown', onKey)
      window.removeEventListener(openSearchEvent, onOpen)
    }
  }, [])

  if (!open) return null
  return <SearchDialog company={company} go={go} onClose={() => setOpen(false)} />
}

function SearchDialog({ company, go, onClose }: { company: Company; go: Go; onClose: () => void }) {
  const [q, setQ] = useState('')
  const [result, setResult] = useState<{ q: string; data: Result } | null>(null)
  const [on, setOn] = useState(0)

  useEffect(() => {
    if (!q.trim()) return
    const timer = window.setTimeout(() => {
      void api<Result>('GET', `/api/v1/c/${company.slug}/search?q=${encodeURIComponent(q)}`).then(
        (data) => {
          setResult({ q, data })
          setOn(0)
        },
        () => undefined,
      )
    }, 150)

    return () => window.clearTimeout(timer)
  }, [q, company.slug])

  const data = q.trim() && result ? result.data : null
  const hits: { key: string; to: string; title: string; sub: string; bg: string }[] = [
    ...(data?.boards ?? []).map((b) => ({ key: `b${b.id}`, to: `/c/${company.slug}/b/${b.id}`, title: b.name, sub: b.project, bg: b.background })),
    ...(data?.cards ?? []).map((c) => ({
      key: `c${c.id}`,
      to: `/c/${company.slug}/b/${c.board.id}/c/${c.id}`,
      title: c.title,
      sub: [c.board.name, c.archived ? t('search.archived') : c.list, c.due_at ? t('search.due', { date: shortDate(c.due_at) }) : ''].filter(Boolean).join(' · '),
      bg: c.board.background,
    })),
  ]

  const pick = (to: string) => {
    onClose()
    go(to)
  }

  const key = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.stopPropagation()
      onClose()
    }

    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setOn((i) => Math.min(i + 1, hits.length - 1))
    }

    if (e.key === 'ArrowUp') {
      e.preventDefault()
      setOn((i) => Math.max(i - 1, 0))
    }

    if (e.key === 'Enter' && hits[on]) {
      e.preventDefault()
      pick(hits[on].to)
    }
  }

  const boards = data?.boards.length ?? 0
  return (
    <div
      className="ov"
      role="dialog"
      aria-modal="true"
      aria-label={t('search.title')}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div className="srch" onKeyDown={key}>
        <input
          type="search"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={t('search.placeholder')}
          aria-label={t('search.title')}
          aria-controls="s-r"
          autoFocus
        />
        <div className="res" id="s-r" role="listbox" aria-label={t('search.results')}>
          {data && boards > 0 && <div className="sub">{t('search.boards')}</div>}
          {data &&
            hits.map((h, i) => (
              <div key={h.key}>
                {i === boards && <div className="sub">{t('search.cards', { n: data.total })}</div>}
                <button type="button" role="option" aria-selected={i === on} className={`hit${i === on ? ' on' : ''}`} onClick={() => pick(h.to)} onMouseEnter={() => setOn(i)}>
                  <span className="bgdot" style={{ background: background(h.bg) }} />
                  <span>
                    <b>{h.title}</b>
                    <small>{h.sub}</small>
                  </span>
                </button>
              </div>
            ))}
          {data && data.cards.length === 0 && (
            <>
              <div className="sub">{t('search.cards', { n: 0 })}</div>
              <p className="muted" style={{ padding: '10px 14px' }}>
                {t('search.none')}
              </p>
            </>
          )}
        </div>
        <p className="hint">
          {t('search.try')} <span className="kbd">@Mai</span> {t('search.member')}, <span className="kbd">#Bug</span> {t('search.label')}, <span className="kbd">is:overdue</span>,{' '}
          <span className="kbd">has:attachments</span>, <span className="kbd">is:archived</span>
        </p>
      </div>
    </div>
  )
}

export function SearchButton() {
  return (
    <button type="button" className="sbtn" aria-label={t('search.title')} onClick={() => window.dispatchEvent(new Event(openSearchEvent))}>
      <svg className="ic" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" aria-hidden="true">
        <circle cx="11" cy="11" r="6.5" />
        <path d="M16 16l4.5 4.5" />
      </svg>
      <span>{t('search.button')}</span>
      <kbd>/</kbd>
    </button>
  )
}
