import { t } from '@/i18n'
import { useMe } from '@/ui/meContext'

export type InviteResult = { result: string; emailed?: boolean; email_limited?: boolean; share_url?: string }

export const wantsEmail = (f: FormData) => f.get('send_email') === 'on'

export const useEmailEnabled = () => Boolean(useMe()?.email_enabled)

export function invitedMessage(res: InviteResult | undefined, email: string, addedKey: 'toast.added' | 'project.toastAdded'): string {
  const who = email.toLowerCase()
  if (res?.result === 'added') return t(addedKey, { email: who })
  if (res?.emailed) return t('invite.emailed', { email: who })
  const url = res?.share_url ?? window.location.origin
  return t(res?.email_limited ? 'invite.limited' : 'invite.share', { email: who, url })
}
