import { useEffect, useState } from 'react'
import { api, HttpError } from '@/http'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'

type Offered = { saved: { id: number; title: string }[]; builtin: { key: string; title: string }[] }

export function CardTemplatePop({ base, list, onMade }: { base: string; list: number; onMade: (id: number) => void }) {
  const [data, setData] = useState<Offered | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let live = true
    api<Offered>('GET', `${base}/card-templates`).then(
      (d) => live && setData(d),
      (e: unknown) => live && setError(e instanceof HttpError ? e.message : String(e)),
    )
    return () => {
      live = false
    }
  }, [base])

  const make = (body: { card?: number; builtin?: string }) =>
    api<{ id: number }>('POST', `${base}/card-templates/use`, { list_id: list, ...body }).then(
      (r) => onMade(r.id),
      (e: unknown) => setError(e instanceof HttpError ? e.message : String(e)),
    )

  return (
    <>
      <p className="hint" style={{ margin: '0 0 8px' }}>
        {t('ctpl.lead')}
      </p>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {data && data.saved.length > 0 && (
        <>
          <div className="pg2">{t('ctpl.saved')}</div>
          {data.saved.map((s) => (
            <button key={s.id} type="button" className="mi" onClick={() => void make({ card: s.id })}>
              <Icon name="tmpl" />
              {s.title}
            </button>
          ))}
        </>
      )}
      {data && (
        <>
          <div className="pg2">{t('ctpl.builtin')}</div>
          {data.builtin.map((b) => (
            <button key={b.key} type="button" className="mi" onClick={() => void make({ builtin: b.key })}>
              <Icon name="tmpl" />
              {b.title}
            </button>
          ))}
        </>
      )}
      <p className="hint" style={{ margin: '10px 0 0' }}>
        {t('ctpl.howTo')}
      </p>
    </>
  )
}
