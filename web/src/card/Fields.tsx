import { useState } from 'react'
import { api } from '@/http'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import type { FieldDef, FieldType, FieldValue } from '@/card/fieldTypes'
import { ENTER } from '@/keys/keyNames'

type Run = (fn: () => Promise<unknown>) => Promise<boolean>

export function FieldControl({
  def: d,
  value: v,
  edit,
  id,
  label,
  withYes = false,
  onSet,
}: {
  def: FieldDef
  value: FieldValue | undefined
  edit: boolean
  id?: string
  label?: string
  withYes?: boolean
  onSet: (value: FieldValue | null) => void
}) {
  if (d.type === 'select')
    return (
      <select id={id} aria-label={label} value={typeof v === 'string' ? v : ''} disabled={!edit} onChange={(e) => onSet(e.target.value || null)}>
        <option value="">—</option>
        {d.options.map((o) => (
          <option key={o}>{o}</option>
        ))}
      </select>
    )
  if (d.type === 'check') {
    const box = <input id={id} aria-label={label} type="checkbox" checked={v === true} disabled={!edit} onChange={(e) => onSet(e.target.checked)} />
    return withYes ? (
      <label className="opt" style={{ height: 34 }}>
        {box}
        {t('fields.yes')}
      </label>
    ) : (
      box
    )
  }

  return (
    <input
      id={id}
      aria-label={label}
      key={`${d.id}:${String(v ?? '')}`}
      type={d.type === 'number' ? 'number' : d.type === 'date' ? 'date' : 'text'}
      defaultValue={v === undefined ? '' : String(v)}
      disabled={!edit}
      onBlur={(e) => {
        const next = e.target.value.trim()
        if (next !== String(v ?? '')) onSet(next === '' ? null : d.type === 'number' ? Number(next) : next)
      }}
      onKeyDown={(e) => {
        if (e.key === ENTER) e.currentTarget.blur()
      }}
    />
  )
}

export function FieldsSection({
  defs,
  values,
  edit,
  path,
  run,
}: {
  defs: FieldDef[]
  values: Record<string, FieldValue>
  edit: boolean
  path: string
  run: Run
}) {
  if (!defs.length) return null
  const set = (id: number, value: FieldValue | null) => void run(() => api('PUT', `${path}/fields/${id}`, { value }))

  return (
    <section className="sec" id="card-fields" aria-label={t('fields.title')}>
      <h3>
        <Icon name="sliders" />
        {t('fields.title')}
      </h3>
      <div className="fgrid">
        {defs.map((d) => {
          const v = values[String(d.id)]
          const id = `fld-${d.id}`
          return (
            <div key={d.id}>
              <label htmlFor={d.type === 'check' ? undefined : id}>{d.name}</label>
              <FieldControl def={d} value={v} edit={edit} id={id} onSet={(value) => set(d.id, value)} withYes />
            </div>
          )
        })}
      </div>
    </section>
  )
}

const TYPES: FieldType[] = ['text', 'number', 'date', 'select', 'check']

function typeLabel(type: FieldType): string {
  return { text: t('fields.type.text'), number: t('fields.type.number'), date: t('fields.type.date'), select: t('fields.type.select'), check: t('fields.type.check') }[type]
}

export function FieldsPage({ defs, edit, base, run }: { defs: FieldDef[]; edit: boolean; base: string; run: Run }) {
  const [name, setName] = useState('')
  const [type, setType] = useState<FieldType>('text')
  const [options, setOptions] = useState('')
  const [front, setFront] = useState(false)

  const add = () => {
    const n = name.trim()
    if (!n) return
    const body = { name: n, type, options: options.split(',').map((o) => o.trim()).filter(Boolean), show_on_card: front }
    void run(() => api('POST', `${base}/fields`, body)).then((ok) => {
      if (!ok) return
      setName('')
      setOptions('')
      setFront(false)
    })
  }

  return (
    <>
      {defs.length === 0 && <p className="hint">{t('fields.none')}</p>}
      {defs.map((d) => (
        <div key={d.id} className="row frow">
          <div style={{ flex: 1 }}>
            <b>{d.name}</b>
            <small className="muted" style={{ display: 'block' }}>
              {d.type === 'select' ? `${typeLabel(d.type)}: ${d.options.join(', ')}` : typeLabel(d.type)}
              {d.show_on_card ? ` · ${t('fields.shownOnCards')}` : ''}
            </small>
          </div>
          {edit && (
            <>
              <button
                type="button"
                className="ibtn sm"
                aria-pressed={d.show_on_card}
                title={t('fields.showOnCards')}
                aria-label={t('fields.toggleFront', { name: d.name })}
                onClick={() => void run(() => api('PATCH', `${base}/fields/${d.id}`, { show_on_card: !d.show_on_card }))}
              >
                <Icon name="eye" />
              </button>
              <button type="button" className="ibtn sm" aria-label={t('fields.delete', { name: d.name })} onClick={() => void run(() => api('DELETE', `${base}/fields/${d.id}`))}>
                <Icon name="trash" />
              </button>
            </>
          )}
        </div>
      ))}
      {edit && (
        <>
          <h4 style={{ margin: '14px 0 4px' }}>{t('fields.add')}</h4>
          <div className="field">
            <label htmlFor="nf-n">{t('fields.name')}</label>
            <input type="text" id="nf-n" placeholder={t('fields.namePlaceholder')} value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div className="field">
            <label htmlFor="nf-t">{t('fields.type')}</label>
            <select id="nf-t" style={{ width: '100%' }} value={type} onChange={(e) => setType(e.target.value as FieldType)}>
              {TYPES.map((x) => (
                <option key={x} value={x}>
                  {typeLabel(x)}
                </option>
              ))}
            </select>
          </div>
          {type === 'select' && (
            <div className="field">
              <label htmlFor="nf-o">{t('fields.options')}</label>
              <input type="text" id="nf-o" placeholder="Low, Medium, High" value={options} onChange={(e) => setOptions(e.target.value)} />
            </div>
          )}
          <label className="opt">
            <input type="checkbox" checked={front} onChange={(e) => setFront(e.target.checked)} />
            {t('fields.showOnCards')}
          </label>
          <button type="button" className="btn primary sm" style={{ marginTop: 6 }} onClick={add}>
            {t('fields.addButton')}
          </button>
        </>
      )}
    </>
  )
}
