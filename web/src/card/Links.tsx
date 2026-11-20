import { useEffect, useState } from 'react'
import { api, errorMessage } from '@/http'
import { ErrorNote } from '@/ui/ErrorNote'
import { t, type MessageKey } from '@/i18n'
import { Icon } from '@/ui/Icon'
import type { CardDetail, CardLink, LinkKind } from '@/card/types'

const LINK_KINDS: LinkKind[] = ['relates', 'blocks', 'blocked_by', 'duplicates', 'duplicated_by']

const SHOWN = 3

const kindText = (k: LinkKind) => t(`links.kind.${k}` as MessageKey)

export function LinksSection({
  card,
  base,
  here,
  edit,
  run,
  onAdd,
  onOpen,
}: {
  card: CardDetail
  base: string
  here: number
  edit: boolean
  run: (fn: () => Promise<unknown>) => Promise<boolean>
  onAdd: (el: Element) => void
  onOpen: (link: CardLink) => void
}) {
  const [all, setAll] = useState(false)
  const list = card.links ?? []
  if (!list.length) return null
  const shown = all ? list : list.slice(-SHOWN)

  return (
    <section className="sec" aria-label={t('links.title')}>
      <h3>
        <Icon name="link" />
        {t('links.title')}
        <span className="sp" />
        {edit && (
          <button type="button" className="btn sm" onClick={(e) => onAdd(e.currentTarget)}>
            {t('common.add')}
          </button>
        )}
      </h3>
      {shown.map((l) => (
        <div className="lkr" key={l.card.id}>
          <span className={`pill${l.kind === 'blocked_by' && !l.card.done && !l.card.archived ? ' bad' : ''}`}>{kindText(l.kind)}</span>
          <button type="button" className="lnk" onClick={() => onOpen(l)}>
            {l.card.title}
          </button>
          <small className="muted">{l.card.archived ? t('links.archived') : l.board.id === here ? l.list : `${l.board.name} · ${l.list}`}</small>
          {l.card.done && <span className="pill ok">{t('links.done')}</span>}
          {edit && (
            <button type="button" className="ibtn sm" aria-label={t('links.remove', { title: l.card.title })} onClick={() => void run(() => api('DELETE', `${base}/cards/${card.id}/links/${l.card.id}`))}>
              <Icon name="x" />
            </button>
          )}
        </div>
      ))}
      {list.length > SHOWN && (
        <button type="button" className="more" aria-expanded={all} onClick={() => setAll(!all)}>
          {all ? t('card.showFewer') : t('links.showAll', { n: list.length })}
        </button>
      )}
    </section>
  )
}

type Target = { card: { id: number; title: string }; board: { id: number; name: string }; list: string }

export function LinkPop({ base, card, onLink }: { base: string; card: number; onLink: (target: number, kind: LinkKind) => void }) {
  const [kind, setKind] = useState<LinkKind>('relates')
  const [q, setQ] = useState('')
  const [found, setFound] = useState<Target[] | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let live = true
    const timer = window.setTimeout(() => {
      api<{ cards: Target[] }>('GET', `${base}/cards/${card}/link-targets?q=${encodeURIComponent(q.trim())}`).then(
        (r) => {
          if (!live) return
          setFound(r.cards)
          setError(null)
        },
        (e: unknown) => live && setError(errorMessage(e)),
      )
    }, 150)

    return () => {
      live = false
      window.clearTimeout(timer)
    }
  }, [base, card, q])

  return (
    <>
      <div className="field">
        <label htmlFor="lk-r">{t('links.thisCard')}</label>
        <select id="lk-r" value={kind} onChange={(e) => setKind(e.target.value as LinkKind)} style={{ width: '100%' }}>
          {LINK_KINDS.map((k) => (
            <option key={k} value={k}>
              {kindText(k)}
            </option>
          ))}
        </select>
      </div>
      <input type="search" value={q} onChange={(e) => setQ(e.target.value)} placeholder={t('links.search')} aria-label={t('links.searchLabel')} data-focus />
      <ErrorNote message={error} />
      <div className="lkres">
        {(found ?? []).map((x) => (
          <button key={x.card.id} type="button" className="mr" onClick={() => onLink(x.card.id, kind)}>
            <span className="lkt">
              <b>{x.card.title}</b>
              <small className="muted">
                {x.board.name} · {x.list}
              </small>
            </span>
          </button>
        ))}
        {found && found.length === 0 && <p className="hint">{t('links.none')}</p>}
      </div>
    </>
  )
}
