import { t } from '@/i18n'
import { Modal } from '@/ui/Modal'
import { Terminal } from '@/tokens/Terminal'

export function SecretModal({ title, lead, label, text, onClose }: { title: string; lead: string; label: string; text: string; onClose: () => void }) {
  return (
    <Modal label={title} onClose={onClose}>
      <h2>{title}</h2>
      <p className="muted">{lead}</p>
      <Terminal text={text} label={label} />
      <div className="acts">
        <button type="button" className="btn primary" onClick={onClose}>
          {t('tokens.done')}
        </button>
      </div>
    </Modal>
  )
}
