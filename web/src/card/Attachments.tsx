import { useEffect, useState } from 'react'
import { api, HttpError, upload } from '@/http'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { ago } from '@/card/look'
import { extOf, fileSize, fileUrl, shortUrl, shownName } from '@/card/files'
import type { CardDetail } from '@/card/types'

const SHOWN = 3

type Storage = { max_file: number; used: number; limit: number | null }

export function AttachmentsSection({
  card,
  base,
  edit,
  run,
  onAdd,
}: {
  card: CardDetail
  base: string
  edit: boolean
  run: (fn: () => Promise<unknown>) => Promise<boolean>
  onAdd: (el: Element) => void
}) {
  const [all, setAll] = useState(false)
  const list = card.attachments
  if (!list.length && !edit) return null
  const shown = all ? list : list.slice(-SHOWN)

  return (
    <section className="sec" aria-label={t('attach.title')}>
      <h3>
        <Icon name="clip" />
        {t('attach.title')}
        <span className="sp" />
        {edit && (
          <button type="button" className="btn sm" onClick={(e) => onAdd(e.currentTarget)}>
            {t('common.add')}
          </button>
        )}
      </h3>
      {list.length === 0 && <p className="hint">{t('attach.none')}</p>}
      {shown.map((a) => {
        const isCover = card.cover_image === a.id
        const href = a.kind === 'link' ? a.url ?? '#' : fileUrl(base, a.id, !a.image)
        return (
          <div className="atr" key={a.id}>
            <a className="thumb" href={href} target="_blank" rel="noopener noreferrer" tabIndex={-1} aria-hidden="true">
              {a.kind === 'link' ? <Icon name="link" /> : a.image ? <img src={fileUrl(base, a.id)} alt="" loading="lazy" /> : extOf(a.name)}
            </a>
            <div className="meta">
              <b title={a.kind === 'link' ? a.url ?? '' : a.name}>
                <a className="alk" href={href} target="_blank" rel="noopener noreferrer">
                  {shownName(a)}
                </a>
              </b>
              <small>
                {a.kind === 'link' ? (a.name.startsWith('http') ? t('attach.link') : shortUrl(a.url ?? '')) : fileSize(a.size)} · {t('attach.added', { when: ago(a.created_at).replace(/^Just/, 'just') })}
              </small>
              {isCover && <span className="pill accent">{t('card.cover')}</span>}
            </div>
            {edit && (
              <div className="row" style={{ flexWrap: 'nowrap' }}>
                {a.image && (
                  <button type="button" className="btn sm" onClick={() => void run(() => api(isCover ? 'DELETE' : 'PUT', `${base}/attachments/${a.id}/cover`))}>
                    {isCover ? t('cardPop.removeCover') : t('attach.makeCover')}
                  </button>
                )}
                <button
                  type="button"
                  className="ibtn sm"
                  aria-label={t('attach.delete', { name: a.name })}
                  onClick={() => void run(() => api('DELETE', `${base}/attachments/${a.id}`))}
                >
                  <Icon name="trash" />
                </button>
              </div>
            )}
          </div>
        )
      })}
      {list.length > SHOWN && (
        <button type="button" className="more" aria-expanded={all} onClick={() => setAll(!all)}>
          {all ? t('card.showFewer') : t('attach.showAll', { n: list.length })}
        </button>
      )}
    </section>
  )
}

export function AttachPop({ base, path, onDone }: { base: string; path: string; onDone: (name: string) => void }) {
  const [storage, setStorage] = useState<Storage | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [url, setUrl] = useState('')
  const [name, setName] = useState('')

  useEffect(() => {
    let live = true
    api<Storage>('GET', `${base.split('/boards/')[0]}/storage`).then(
      (s) => live && setStorage(s),
      () => undefined,
    )
    return () => {
      live = false
    }
  }, [base])

  const send = async (work: () => Promise<unknown>, label: string) => {
    setBusy(true)
    setError(null)
    try {
      await work()
      onDone(label)
    } catch (e) {
      setError(e instanceof HttpError ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }

  const pick = (file: File | undefined) => {
    if (!file) return
    if (storage && file.size > storage.max_file) {
      setError(t('attach.tooLarge', { size: fileSize(storage.max_file) }))
      return
    }

    void send(() => upload(`${path}/attachments`, file), file.name)
  }

  const link = () => {
    if (!/^https?:\/\/\S+$/i.test(url.trim())) {
      setError(t('attach.badLink'))
      return
    }

    void send(() => api('POST', `${path}/attachments/link`, { url: url.trim(), name: name.trim() || undefined }), name.trim() || url.trim())
  }

  const max = storage ? fileSize(storage.max_file) : '10 MB'
  return (
    <>
      <label className="mi" style={{ cursor: busy ? 'progress' : 'pointer' }}>
        <Icon name="upload" />
        <span>{busy ? t('attach.uploading') : t('attach.upload')}</span>
        <input type="file" style={{ display: 'none' }} disabled={busy} aria-label={t('attach.upload')} onChange={(e) => pick(e.target.files?.[0])} />
      </label>
      <p className="hint" style={{ margin: '2px 8px 10px' }}>
        {storage?.limit
          ? t('attach.hintLimit', { max, used: fileSize(storage.used), limit: fileSize(storage.limit) })
          : t('attach.hint', { max })}
      </p>
      {error && (
        <p className="error" role="alert" style={{ margin: '0 8px 10px' }}>
          {error}
        </p>
      )}
      <div className="field">
        <label htmlFor="at-u">{t('attach.pasteLink')}</label>
        <input type="text" id="at-u" placeholder="https://" value={url} onChange={(e) => setUrl(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && link()} autoFocus />
      </div>
      <div className="field">
        <label htmlFor="at-n">{t('attach.linkName')}</label>
        <input type="text" id="at-n" value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && link()} />
      </div>
      <button type="button" className="btn primary sm" disabled={busy} onClick={link}>
        {t('attach.attachLink')}
      </button>
    </>
  )
}
