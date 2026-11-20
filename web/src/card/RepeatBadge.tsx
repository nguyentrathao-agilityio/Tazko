import { t } from '@/i18n'
import { ruleText } from '@/card/repeat'
import { Icon } from '@/ui/Icon'

export function RepeatBadge({ rule }: { rule: string | null | undefined }) {
  if (!rule) return null
  return (
    <span className="bdg" title={t('repeat.badge', { rule: ruleText(rule) })}>
      <Icon name="repeat" />
    </span>
  )
}
