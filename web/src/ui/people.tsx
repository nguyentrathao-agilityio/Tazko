import { t } from '@/i18n'
import { displayName, initials, tone } from '@/ui/names'

export function Avatar({ name, square = false, big = false }: { name: string; square?: boolean; big?: boolean }) {
  return (
    <span className={`av ${tone(name)}${square ? ' sq' : ''}${big ? ' big' : ''}`} aria-hidden="true">
      {initials(name)}
    </span>
  )
}

export function AccountPill({ email }: { email: string }) {
  return /@(gmail|googlemail)\.com$/i.test(email) ? (
    <span className="pill">{t('account.personal')}</span>
  ) : (
    <span className="pill accent">{t('account.workspace')}</span>
  )
}

export function Person({ name, email }: { name: string; email: string }) {
  const shown = displayName(name, email)
  return (
    <div className="who">
      <Avatar name={shown} />
      <div>
        <b>{shown}</b>
        {shown !== email && <small>{email}</small>}
      </div>
    </div>
  )
}
