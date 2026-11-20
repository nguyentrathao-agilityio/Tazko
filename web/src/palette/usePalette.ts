import { useEffect, useState } from 'react'
import { saveSettings, useSettings } from '@/settings/store'
import type { Go } from '@/ui/Link'
import { openSearchEvent } from '@/ui/typing'
import type { Personal } from '@/palette/commands'

const BLOCKING = '.pop, .modal, .drawer, .ov:not(.pal):not(.cmov)'

export function usePalette(enabled = true) {
  const [open, setOpen] = useState(false)

  useEffect(() => {
    if (!enabled) return
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key.toLowerCase() !== 'k' || !(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return
      if (document.querySelector(BLOCKING)) return
      e.preventDefault()
      setOpen((o) => !o)
    }
    window.addEventListener('keydown', onKey)

    return () => window.removeEventListener('keydown', onKey)
  }, [enabled])

  return { open: enabled && open, show: () => setOpen(true), close: () => setOpen(false) }
}

export function usePersonal(go: Go, companyRoot: string): Personal {
  const settings = useSettings()
  return {
    density: settings.zen_density,
    quiet: settings.zen_quiet,
    setDensity: (d) => void saveSettings({ zen_density: d }).catch(() => undefined),
    toggleQuiet: () => void saveSettings({ zen_quiet: !settings.zen_quiet }).catch(() => undefined),
    search: () => window.dispatchEvent(new Event(openSearchEvent)),
    go,
    companyRoot,
  }
}
