import { useEffect, useRef, type ReactNode } from 'react'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import type { Anchor } from '@/ui/anchor'

export function Popover({
  anchor,
  title,
  width = 320,
  onClose,
  onBack,
  children,
}: {
  anchor: Anchor
  title: string
  width?: number
  onClose: () => void
  onBack?: () => void
  children: ReactNode
}) {
  const box = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const before = document.activeElement as HTMLElement | null
    const first = box.current?.querySelector<HTMLElement>('[data-focus],input,select,textarea,.pop-b button')
    first?.focus()
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation()
        onClose()
      }
    }
    const onDown = (e: MouseEvent) => {
      if (box.current && !box.current.contains(e.target as Node)) onClose()
    }
    window.addEventListener('keydown', onKey, true)
    document.addEventListener('mousedown', onDown)

    return () => {
      window.removeEventListener('keydown', onKey, true)
      document.removeEventListener('mousedown', onDown)
      before?.focus?.()
    }
  }, [onClose])

  const vw = typeof window === 'undefined' ? 1024 : window.innerWidth
  const left = Math.max(8, Math.min(anchor.left, vw - width - 8))
  return (
    <div className="pop" role="dialog" aria-label={title} ref={box} style={{ left, top: anchor.bottom + 6, width }}>
      <div className="pop-h">
        {onBack ? (
          <button type="button" className="ibtn sm" onClick={onBack} aria-label={t('pop.back')}>
            <Icon name="back" />
          </button>
        ) : (
          <span style={{ width: 26 }} />
        )}
        <b>{title}</b>
        <button type="button" className="ibtn sm" onClick={onClose} aria-label={t('pop.close')}>
          <Icon name="x" />
        </button>
      </div>
      <div className="pop-b">{children}</div>
    </div>
  )
}
