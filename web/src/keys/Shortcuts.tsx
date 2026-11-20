import { useEffect } from 'react'
import { t, type MessageKey } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { ESCAPE } from '@/keys/keyNames'

const keys: [string, MessageKey][] = [
  ['` ?', 'keys.help'],
  ['Z', 'keys.zen'],
  ['Ctrl K', 'keys.palette'],
  ['/', 'keys.search'],
  ['F', 'keys.filter'],
  ['Q', 'keys.mine'],
  ['N', 'keys.new'],
  ['J K', 'keys.updown'],
  ['H L', 'keys.leftright'],
  ['Enter', 'keys.open'],
  ['E', 'keys.quick'],
  ['Space', 'keys.done'],
  ['A', 'keys.members'],
  ['D', 'keys.dates'],
  ['T', 'keys.labels'],
  ['M', 'keys.move'],
  ['X', 'keys.archive'],
  ['1–8', 'keys.label'],
  ['G', 'keys.drag'],
  ['J K H L', 'keys.cardOpen'],
  ['Alt ↑ ↓', 'keys.item'],
  ['Esc', 'keys.escape'],
]

export function Shortcuts({ onClose }: { onClose: () => void }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === ESCAPE || e.key === '`' || e.key === '?') {
        e.preventDefault()
        e.stopPropagation()
        onClose()
      }
    }
    window.addEventListener('keydown', onKey, true)

    return () => window.removeEventListener('keydown', onKey, true)
  }, [onClose])

  return (
    <div
      className="ov"
      role="dialog"
      aria-modal="true"
      aria-label={t('keys.title')}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div className="srch" style={{ padding: '18px 22px' }}>
        <div className="row">
          <h2 style={{ flex: 1 }}>{t('keys.title')}</h2>
          <button type="button" className="ibtn" aria-label={t('pop.close')} onClick={onClose}>
            <Icon name="x" />
          </button>
        </div>
        <p className="hint" style={{ margin: '0 0 10px', padding: 0, border: 0 }}>
          {t('keys.lead')}
        </p>
        <dl className="kt">
          {keys.map(([k, label]) => (
            <div key={k} style={{ display: 'contents' }}>
              <dt>
                <span className="kbd">{k}</span>
              </dt>
              <dd>{t(label)}</dd>
            </div>
          ))}
        </dl>
      </div>
    </div>
  )
}
