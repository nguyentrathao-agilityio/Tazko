import { t } from '@/i18n'
import { useEmailEnabled } from '@/invite/invite'

export function EmailInviteOption() {
  if (!useEmailEnabled()) return null

  return (
    <label className="opt" style={{ margin: 0 }}>
      <input type="checkbox" name="send_email" defaultChecked />
      {t('invite.emailOption')}
    </label>
  )
}
