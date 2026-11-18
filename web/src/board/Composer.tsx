import { useState, type FormEvent, type KeyboardEvent } from 'react'
import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { splitTitles } from '@/board/titles'

export function Composer({ open, onOpen, onClose, onAdd }: { open: boolean; onOpen: () => void; onClose: () => void; onAdd: (titles: string[]) => Promise<boolean> }) {
  const [text, setText] = useState('')

  const submit = (e?: FormEvent) => {
    e?.preventDefault()
    const titles = splitTitles(text)
    if (titles.length === 0) return
    void onAdd(titles).then((ok) => {
      if (ok) setText('')
    })
  }

  const key = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      submit()
    }

    if (e.key === 'Escape') {
      e.stopPropagation()
      onClose()
    }
  }

  if (!open)
    return (
      <div className="lf">
        <button type="button" className="addc" onClick={onOpen}>
          <Icon name="plus" />
          {t('card.add')}
        </button>
      </div>
    )

  const lines = splitTitles(text).length
  return (
    <form className="cmp" onSubmit={submit}>
      <textarea value={text} onChange={(e) => setText(e.target.value)} onKeyDown={key} placeholder={t('card.titlePlaceholder')} aria-label={t('card.title')} autoFocus />
      {lines > 1 && <p className="hint" style={{ padding: '4px 0 0', border: 0 }}>{t('card.manyLines', { n: lines })}</p>}
      <div className="row">
        <button type="submit" className="btn primary sm">
          {lines > 1 ? t('card.addMany', { n: lines }) : t('card.addButton')}
        </button>
        <button type="button" className="ibtn sm" aria-label={t('common.cancel')} onClick={onClose}>
          <Icon name="x" />
        </button>
      </div>
    </form>
  )
}
