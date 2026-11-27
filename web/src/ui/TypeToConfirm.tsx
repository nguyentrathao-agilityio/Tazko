import { useState } from 'react'
import { api, errorMessage } from '@/http'
import { t } from '@/i18n'
import { ErrorNote } from '@/ui/ErrorNote'
import { Modal } from '@/ui/Modal'

export function TypeToConfirm({
  title,
  lead,
  prompt,
  word,
  url,
  go,
  sameCase = true,
  onClose,
  onDone,
}: {
  title: string
  lead: string
  prompt: string
  word: string
  url: string
  go: string
  sameCase?: boolean
  onClose: () => void
  onDone: () => void
}) {
  const [typed, setTyped] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [before, after] = prompt.split('{word}')
  const norm = (s: string) => (sameCase ? s.trim() : s.trim().toLowerCase())

  const submit = async () => {
    try {
      await api('DELETE', url, { confirm: typed })
      onDone()
    } catch (e) {
      setError(errorMessage(e))
    }
  }

  return (
    <Modal label={title} onClose={onClose}>
      <h2>{title}</h2>
      <p className="muted">{lead}</p>
      <div className="field">
        <label htmlFor="ttc-word">
          {before}
          <b className="mono">{word}</b>
          {after}
        </label>
        <input type="text" id="ttc-word" autoComplete="off" value={typed} onChange={(e) => setTyped(e.target.value)} />
      </div>
      <ErrorNote message={error} />
      <div className="acts">
        <button type="button" className="btn" onClick={onClose}>
          {t('common.cancel')}
        </button>
        <button type="button" className="btn danger" disabled={norm(typed) !== norm(word)} onClick={() => void submit()}>
          {go}
        </button>
      </div>
    </Modal>
  )
}
