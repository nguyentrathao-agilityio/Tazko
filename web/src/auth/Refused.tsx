import { useEffect, useState } from 'react'
import { getRefusal, signInHref, type RefusalNote } from '@/api'
import { t, type MessageKey } from '@/i18n'
import { AuthCard, GoogleButton, Note } from '@/auth/AuthCard'

type Reason = { title: MessageKey; lead: MessageKey; unknown: MessageKey; note: MessageKey }

const reasons: Record<string, Reason> = {
  not_invited: { title: 'refused.title', lead: 'refused.lead', unknown: 'refused.lead.unknown', note: 'refused.note.not_invited' },
  invite_expired: { title: 'refused.title.expired', lead: 'refused.lead.expired', unknown: 'refused.unknown.expired', note: 'refused.note.invite_expired' },
  disabled: { title: 'refused.title.disabled', lead: 'refused.lead.disabled', unknown: 'refused.unknown.disabled', note: 'refused.note.disabled' },
  email_not_verified: { title: 'refused.title.unverified', lead: 'refused.lead.unverified', unknown: 'refused.unknown.unverified', note: 'refused.note.email_not_verified' },
  account_mismatch: { title: 'refused.title.mismatch', lead: 'refused.lead.mismatch', unknown: 'refused.unknown.mismatch', note: 'refused.note.account_mismatch' },
}

export function Refused() {
  const [note, setNote] = useState<RefusalNote | null>(null)
  const [loaded, setLoaded] = useState(false)

  useEffect(() => {
    const ctrl = new AbortController()
    getRefusal(ctrl.signal)
      .then(setNote)
      .then(() => setLoaded(true))
      .catch(() => setLoaded(true))

    return () => ctrl.abort()
  }, [])

  const reason = reasons[note?.reason ?? ''] ?? reasons.not_invited
  const [before, after] = t(reason.lead).split('{email}')

  return (
    <AuthCard>
      <h1 className="text-2xl font-bold">{loaded ? t(reason.title) : t('refused.title.loading')}</h1>
      <p className="mt-2 text-muted">
        {note?.email ? (
          <>
            {before}
            <b className="text-ink">{note.email}</b>
            {after}
          </>
        ) : (
          loaded && t(reason.unknown)
        )}
      </p>
      {loaded && <Note>{t(reason.note)}</Note>}
      <GoogleButton href={signInHref()}>{t('refused.retry')}</GoogleButton>
    </AuthCard>
  )
}
