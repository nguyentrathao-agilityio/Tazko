import { signInHref } from '@/api'
import { t, type MessageKey } from '@/i18n'
import { AuthCard, GoogleButton, Note } from '@/auth/AuthCard'

const errors: Record<string, MessageKey> = {
  expired: 'signin.error.expired',
  failed: 'signin.error.failed',
  cancelled: 'signin.error.cancelled',
}

export function SignIn({ search }: { search: string }) {
  const params = new URLSearchParams(search)
  const error = errors[params.get('error') ?? '']
  const next = params.get('next') ?? undefined

  return (
    <AuthCard>
      <h1 className="text-2xl font-bold">{t('signin.title')}</h1>
      <p className="mt-2 text-muted">{t('signin.lead')}</p>
      {error && (
        <p role="alert" className="mt-4 rounded-[10px] bg-[var(--bad-bg)] px-3.5 py-2.5 text-left text-[.85rem] text-[var(--bad)]">
          {t(error)}
        </p>
      )}
      <GoogleButton href={signInHref(next)}>{t('signin.google')}</GoogleButton>
      <Note>{t('signin.note')}</Note>
    </AuthCard>
  )
}
