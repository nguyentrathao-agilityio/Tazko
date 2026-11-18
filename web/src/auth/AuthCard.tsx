import type { ReactNode } from 'react'
import { Mark } from '@/ui/Mark'

export function AuthCard({ children }: { children: ReactNode }) {
  return (
    <div className="grid min-h-screen place-items-center px-4 py-10">
      <div className="w-full max-w-[420px] rounded-2xl border border-line bg-surface px-7 py-8 text-center shadow-[var(--shadow)]">
        <div className="mb-[18px] flex justify-center">
          <Mark />
        </div>
        {children}
      </div>
    </div>
  )
}

export function GoogleButton({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a
      href={href}
      className="mt-[22px] flex h-11 w-full items-center justify-center gap-3 rounded-[10px] border border-line bg-surface text-[.95rem] font-semibold text-ink no-underline hover:bg-sunken"
    >
      <span
        aria-hidden="true"
        className="inline-grid size-5 place-items-center rounded-full"
        style={{ background: 'conic-gradient(#ea4335 0 25%,#fbbc05 0 50%,#34a853 0 75%,#4285f4 0)' }}
      >
        <i className="block size-3 rounded-full bg-surface" />
      </span>
      {children}
    </a>
  )
}

export function Note({ children }: { children: ReactNode }) {
  return <div className="mt-4 rounded-[10px] bg-sunken px-3.5 py-2.5 text-left text-[.85rem] text-muted">{children}</div>
}
