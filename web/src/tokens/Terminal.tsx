import { t } from '@/i18n'
import { Icon } from '@/ui/Icon'
import { useToast } from '@/ui/toast'

export function Terminal({ text, label }: { text: string; label: string }) {
  const toast = useToast()
  const copy = () => {
    if (!navigator.clipboard) {
      toast(t('tokens.copyBlocked'))
      return
    }

    void navigator.clipboard.writeText(text).then(
      () => toast(t('tokens.copied')),
      () => toast(t('tokens.copyBlocked')),
    )
  }
  return (
    <div className="termw">
      <pre className="term" aria-label={label}>
        {text}
      </pre>
      <button type="button" className="btn sm termc" onClick={copy}>
        <Icon name="copy" />
        {t('tokens.copy')}
      </button>
    </div>
  )
}
