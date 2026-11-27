import type { MouseEvent, ReactNode } from 'react'

export type Go = (to: string, replace?: boolean) => void

export function Link({ to, go, className, label, children }: { to: string; go: Go; className?: string; label?: string; children: ReactNode }) {
  const onClick = (e: MouseEvent<HTMLAnchorElement>) => {
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0) return
    e.preventDefault()
    go(to)
  }

  return (
    <a href={to} className={className} aria-label={label} onClick={onClick}>
      {children}
    </a>
  )
}
