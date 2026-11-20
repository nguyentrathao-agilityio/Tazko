import { useEffect, useState } from 'react'
import { popoverOpen } from '@/ui/typing'

export function usePalette() {
  const [open, setOpen] = useState(false)

  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key.toLowerCase() !== 'k' || !(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return
      if (popoverOpen() && !document.querySelector('.ov.pal')) return
      e.preventDefault()
      setOpen((o) => !o)
    }
    window.addEventListener('keydown', onKey)

    return () => window.removeEventListener('keydown', onKey)
  }, [])

  return { open, show: () => setOpen(true), close: () => setOpen(false) }
}
