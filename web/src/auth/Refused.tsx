import { useEffect, useState } from 'react'
import { getRefusal, signInHref, type RefusalNote } from '@/api'
import { t, type MessageKey } from '@/i18n'
import { AuthCard, GoogleButton, Note } from '@/auth/AuthCard'

const notes: Record<string, MessageKey> = {
  not_invited: 'refused.note.not_invited',
  email_not_verified: 'refused.note.email_not_verified',
  disabled: 'refused.note.disabled',
  account_mismatch: 'refused.note.account_mismatch',
}

export function Refused() {
  const [note, setNote] = useState<RefusalNote | null>(null)

  useEffect(() => {
    const ctrl = new AbortController()
    getRefusal(ctrl.signal)
      .then(setNote)
      .catch(() => undefined)

    return () => ctrl.abort()
  }, [])

  const [before, after] = t('refused.lead').split('{email}')

  return (
    <AuthCard>
      <h1 className="text-2xl font-bold">{t('refused.title')}</h1>
      <p className="mt-2 text-muted">
        {note?.email ? (
          <>
            {before}
            <b className="text-ink">{note.email}</b>
            {after}
          </>
        ) : (
          t('refused.lead.unknown')
        )}
      </p>
      <Note>{t(notes[note?.reason ?? ''] ?? 'refused.note.not_invited')}</Note>
      <GoogleButton href={signInHref()}>{t('refused.retry')}</GoogleButton>
    </AuthCard>
  )
}
