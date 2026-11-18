import type { ReactNode } from 'react'

export function BoardsPicture() {
  return (
    <svg width="120" height="80" viewBox="0 0 120 80" aria-hidden="true" className="mx-auto mb-3.5 block">
      <rect x="6" y="8" width="30" height="64" rx="8" fill="var(--accent-soft)" />
      <rect x="45" y="8" width="30" height="42" rx="8" fill="var(--accent-line)" />
      <rect x="84" y="8" width="30" height="54" rx="8" fill="var(--accent-soft)" />
    </svg>
  )
}

export function EmptyState({
  title,
  body,
  picture,
  children,
}: {
  title: string
  body?: string
  picture?: ReactNode
  children?: ReactNode
}) {
  return (
    <div className="rounded-xl border-[1.5px] border-dashed border-line bg-surface px-4 py-14 text-center">
      {picture}
      <h2>{title}</h2>
      {body && <p className="mx-auto mt-1.5 max-w-[48ch] text-muted">{body}</p>}
      {children && <div className="mt-4 flex flex-wrap justify-center gap-2.5">{children}</div>}
    </div>
  )
}
