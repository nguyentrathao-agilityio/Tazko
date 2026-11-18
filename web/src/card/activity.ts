import type { Label, ListData } from '@/board/types'
import type { ActivityEntry } from '@/card/types'

type Ctx = { lists: ListData[]; labels: Label[]; people: { id: number; name: string; email: string }[] }

function str(v: unknown): string {
  return typeof v === 'string' ? v : ''
}

function num(v: unknown): number | null {
  return typeof v === 'number' ? v : null
}

export function describe(e: ActivityEntry, ctx: Ctx): string {
  const p = e.payload ?? {}
  const list = (id: unknown) => ctx.lists.find((l) => l.id === num(id))?.title ?? 'another list'
  const label = (id: unknown) => ctx.labels.find((l) => l.id === num(id))?.name || 'a label'
  const who = (id: unknown) => {
    const person = ctx.people.find((x) => x.id === num(id))
    return person ? person.name || person.email : 'someone'
  }

  switch (e.type) {
    case 'card_created':
      return `added this card to ${list(p.list)}`
    case 'card_copied':
      return 'copied this card'
    case 'card_renamed':
      return `renamed this card to “${str(p.to) || str(p.title)}”`
    case 'card_moved':
      return p.from_list === p.to_list ? `moved this card within ${list(p.to_list)}` : `moved this card from ${list(p.from_list)} to ${list(p.to_list)}`
    case 'card_archived':
      return 'archived this card'
    case 'card_restored':
      return 'restored this card'
    case 'card_done':
      return p.done ? 'marked this card complete' : 'marked this card incomplete'
    case 'card_updated':
      if ('description' in p) return 'updated the description'
      if ('due_at' in p || 'start_at' in p) return 'changed the dates'
      if ('cover' in p) return 'changed the cover'
      return 'updated this card'
    case 'label_added':
      return `added the ${label(p.label)} label`
    case 'label_removed':
      return `removed the ${label(p.label)} label`
    case 'member_assigned':
      return `added ${who(p.user)} to this card`
    case 'member_unassigned':
      return `removed ${who(p.user)} from this card`
    case 'checklist_added':
      return `added the checklist ${str(p.title)}`
    case 'checklist_renamed':
      return `renamed a checklist to ${str(p.title)}`
    case 'checklist_deleted':
      return 'deleted a checklist'
    case 'checklist_moved':
      return 'reordered the checklists'
    case 'item_added':
      return `added “${str(p.text)}” to a checklist`
    case 'item_checked':
      return 'completed a checklist item'
    case 'item_unchecked':
      return 'reopened a checklist item'
    case 'item_changed':
      return 'changed a checklist item'
    case 'item_deleted':
      return 'deleted a checklist item'
    case 'item_moved':
      return 'reordered a checklist'
    case 'comment_added':
      return 'commented on this card'
    case 'comment_edited':
      return 'edited a comment'
    case 'comment_deleted':
      return 'deleted a comment'
    default:
      return e.type.replace(/_/g, ' ')
  }
}
