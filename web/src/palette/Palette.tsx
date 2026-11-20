import { useEffect, useRef, useState, type KeyboardEvent } from 'react'
import { t, type MessageKey } from '@/i18n'
import { ARROW_DOWN, ARROW_UP, ENTER, ESCAPE, TAB } from '@/keys/keyNames'
import { type Command, narrow } from '@/palette/commands'

export function Palette({ items, onClose }: { items: Command[]; onClose: () => void }) {
  const [q, setQ] = useState('')
  const [on, setOn] = useState(0)
  const list = useRef<HTMLDivElement>(null)
  const shown = narrow(items, q)
  const at = Math.min(on, Math.max(shown.length - 1, 0))

  useEffect(() => {
    list.current?.querySelector('.hit.on')?.scrollIntoView?.({ block: 'nearest' })
  }, [at, q])

  const runAt = (i: number) => {
    const c = shown[i]
    if (!c) return
    onClose()
    window.setTimeout(c.run, 0)
  }

  const key = (e: KeyboardEvent) => {
    const n = shown.length
    if (e.nativeEvent.isComposing) return
    if (e.key === TAB) e.preventDefault()

    if (e.key === ESCAPE) {
      e.stopPropagation()
      onClose()
    }

    if ((e.key === ARROW_DOWN || e.key === ARROW_UP) && n) {
      e.preventDefault()
      setOn((at + (e.key === ARROW_DOWN ? 1 : -1) + n) % n)
    }

    if (e.key === ENTER) {
      e.preventDefault()
      runAt(at)
    }
  }

  return (
    <div
      className="ov pal"
      role="dialog"
      aria-modal="true"
      aria-label={t('palette.title')}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div className="srch" onKeyDown={key}>
        <input
          type="search"
          value={q}
          onChange={(e) => {
            setQ(e.target.value)
            setOn(0)
          }}
          placeholder={t('palette.placeholder')}
          aria-label={t('palette.title')}
          aria-controls="pal-r"
          aria-activedescendant={shown[at] ? `pal-${shown[at].id}` : undefined}
          autoComplete="off"
          autoFocus
        />
        <div className="res" id="pal-r" role="listbox" aria-label={t('palette.title')} ref={list}>
          {shown.map((c, i) => (
            <div key={c.id}>
              {(i === 0 || shown[i - 1].group !== c.group) && <div className="sub">{t(`palette.group.${c.group}` as MessageKey)}</div>}
              <button type="button" id={`pal-${c.id}`} role="option" aria-selected={i === at} tabIndex={-1} className={`hit${i === at ? ' on' : ''}`} onClick={() => runAt(i)} onMouseMove={() => i !== at && setOn(i)}>
                <span className="pl">
                  <b>{c.label}</b>
                </span>
                {c.hint && <small className="pg">{c.hint}</small>}
                {c.keys && <span className="kbd">{c.keys}</span>}
              </button>
            </div>
          ))}
          {shown.length === 0 && (
            <p className="muted" style={{ padding: '12px 14px' }}>
              {t('palette.none')}
            </p>
          )}
        </div>
        <p className="hint">
          <span className="kbd">↑</span> <span className="kbd">↓</span> {t('palette.choose')}, <span className="kbd">Enter</span> {t('palette.run')}, <span className="kbd">Esc</span> {t('palette.close')}
        </p>
      </div>
    </div>
  )
}
