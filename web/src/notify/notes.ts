import { t } from '@/i18n'

export type Note = {
  id: number
  kind: 'mention' | 'assigned' | 'due' | 'card_added'
  actor: { id: number; name: string; email: string | null } | null
  card: { id: number; title: string }
  board: { id: number; name: string }
  created_at: number
  read: boolean
  due_at: number | null
}

export function companyOf(path: string): string | null {
  const m = /^\/c\/([a-z0-9-]+)(\/|$)/.exec(path)
  return m ? m[1] : null
}

export function noteText(n: Note): { who: string; what: string } {
  const who = n.actor ? n.actor.name || n.actor.email || t('card.someone') : ''
  const what = {
    mention: t('notify.mention'),
    assigned: t('notify.assigned'),
    due: t('notify.due'),
    card_added: t('notify.cardAdded', { board: n.board.name }),
  }[n.kind]
  return { who, what }
}
