import { useEffect } from 'react'
import { useSettings } from '@/settings/store'

export function useApplySettings() {
  const s = useSettings()

  useEffect(() => {
    const root = document.documentElement
    if (s.theme === 'system') root.removeAttribute('data-theme')
    else root.setAttribute('data-theme', s.theme)
    const zoom = s.zoom / 100
    root.style.setProperty('--zm', String(zoom))
    root.style.zoom = zoom === 1 ? '' : String(zoom)
    root.classList.toggle('dense', s.density === 'compact')
    root.classList.toggle('nomo', s.reduce_motion)
    root.classList.toggle('lbc', s.label_style === 'bars')
  }, [s.theme, s.zoom, s.density, s.reduce_motion, s.label_style])
}
