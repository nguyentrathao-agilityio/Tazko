import type { ReactNode } from 'react'

export function PageHead({ title, lead, children }: { title: ReactNode; lead?: ReactNode; children?: ReactNode }) {
  return (
    <div className="head">
      <div>
        <h1>{title}</h1>
        {lead && <p className="muted">{lead}</p>}
      </div>
      {children && (
        <>
          <span className="sp" />
          {children}
        </>
      )}
    </div>
  )
}
