import { useEffect, useRef, type ReactNode } from 'react'
import { ESCAPE } from '@/keys/keyNames'

export function Modal({ label, onClose, children }: { label: string; onClose: () => void; children: ReactNode }) {
  const sheet = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const before = document.activeElement as HTMLElement | null
    sheet.current?.querySelector<HTMLElement>('input,select,button')?.focus()
    const onKey = (e: KeyboardEvent) => {
      if (e.key === ESCAPE) onClose()
    }
    window.addEventListener('keydown', onKey)

    return () => {
      window.removeEventListener('keydown', onKey)
      before?.focus()
    }
  }, [onClose])

  return (
    <div className="modal">
      <div className="scrim" onClick={onClose} />
      <div className="sheet" role="dialog" aria-modal="true" aria-label={label} ref={sheet}>
        {children}
      </div>
    </div>
  )
}
